// Vertical slice (openspec/changes/http-search-vertical-slice, http-browse-vertical-slice,
// signed-in-http-parity): proves crates/innertube/crates/core work behind a plain HTTP server,
// reachable from a browser tab instead of Tauri's IPC.
mod auth;
mod blocked;
mod browse;
mod diagnostics;
mod lastfm;
mod library;
mod lyrics;
mod media;
mod playlists;
mod ratings;
mod search;
mod session;
mod settings;
mod ws;

use std::sync::Arc;

use axum::http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode};
use limusic_core::state::AppState;
use innertube::{Clients, InnerTube, Session};
use tower_http::cors::CorsLayer;

/// Everything a route handler needs: the forked `crates/core` state (account/library/settings
/// logic) plus the session store `crates/core` deliberately doesn't own (design.md's
/// "Enforcement" decision). `Clone` is cheap — both fields are already `Arc`-backed internally.
#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) app: Arc<AppState>,
    pub(crate) sessions: Arc<session::SessionStore>,
}

pub(crate) type ApiError = (StatusCode, String);

/// Shared by every route: the anonymous metadata client every `innertube` call in this server
/// uses. Factored out once it's needed by more than the original single route.
pub(crate) fn metadata_client(state: &AppState) -> Result<&innertube::YouTubeClient, ApiError> {
    state
        .clients
        .get(innertube::METADATA_CLIENT)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "metadata client missing".to_owned()))
}

/// Shared by every route taking a required text query: trims it and rejects a missing/empty one
/// with `400`, matching `/api/search`'s original behavior.
pub(crate) fn require_query(q: Option<String>) -> Result<String, ApiError> {
    q.map(|q| q.trim().to_owned())
        .filter(|q| !q.is_empty())
        .ok_or((StatusCode::BAD_REQUEST, "missing or empty `q` query parameter".to_owned()))
}

/// Map a plain `Result<_, String>` from a `crates/core` command into an `ApiError`. Most of this
/// server's signed-in routes are thin wrappers over those commands, whose errors are already
/// human-readable strings (sign-in failures, "unknown setting: x", etc.) — `400` fits better than
/// the anonymous routes' uniform `502`, since these are far more often a bad request than an
/// upstream hiccup.
pub(crate) fn command_err(message: String) -> ApiError {
    (StatusCode::BAD_REQUEST, message)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // A dedicated SQLite file for this server — never the desktop app's own `limusic.sqlite`
    // (that file is `src-tauri`'s, opened by a Tauri-managed `AppState` this process never
    // touches). Fixed relative path for now: this server is still a `cargo run` development
    // artifact (see http-search-vertical-slice design.md's hosting Non-Goals), not yet something
    // with its own configuration story.
    let db_path = std::path::Path::new("limusic-server.sqlite");
    let (db, quarantined) =
        limusic_core::db::Db::open_or_quarantine(db_path).expect("failed to open limusic-server.sqlite");
    if let Some(aside) = quarantined {
        tracing::warn!(path = %aside.display(), "limusic-server.sqlite was corrupt; quarantined");
    }
    let db = Arc::new(db);

    let lastfm = limusic_core::lastfm::spawn(db.get_setting("lastfm_session_key"));

    // Same "cargo run development artifact" stance as the sqlite path above — a fixed relative
    // directory, not yet a real configuration story (http-search-vertical-slice design.md).
    let app_data_dir = std::path::Path::new("limusic-server-data");
    std::fs::create_dir_all(app_data_dir).expect("failed to create limusic-server-data");

    let app_state = Arc::new(AppState::new(
        InnerTube::new(Session::default(), None).expect("failed to build InnerTube client"),
        Clients::bundled(),
        db,
        lastfm,
        app_data_dir,
    ));
    let state =
        ServerState { app: app_state, sessions: Arc::new(session::SessionStore::default()) };

    // Permissive but explicit, not `CorsLayer::permissive()`: a named origin is one line more and
    // doesn't invite copy-pasting a wildcard into a later, less throwaway version of this file.
    // `allow_credentials` is needed now that routes set/read a session cookie.
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5183".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers([CONTENT_TYPE])
        .allow_credentials(true);

    let app = search::router()
        .merge(browse::router())
        .merge(auth::router())
        .merge(ws::router())
        .merge(library::router())
        .merge(playlists::router())
        .merge(ratings::router())
        .merge(settings::router())
        .merge(lyrics::router())
        .merge(lastfm::router())
        .merge(blocked::router())
        .merge(diagnostics::router())
        .merge(media::router())
        .with_state(state)
        .layer(cors);

    // Defaults to the existing localhost-only dev behavior; a container deployment sets
    // BIND_ADDR=0.0.0.0:8787 so Docker's port publishing (itself bound to 127.0.0.1 on the host,
    // see deploy/docker-compose.yml) can reach the process from outside its network namespace.
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_owned());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    tracing::info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
