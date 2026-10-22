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
#[cfg(test)]
mod tests;
mod ws;

use std::sync::Arc;

use axum::http::{header::CONTENT_TYPE, HeaderValue, Method, StatusCode};
use limusic_core::state::Shared;
use innertube::Clients;
use tower_http::cors::CorsLayer;

/// Everything a route handler needs: the forked `crates/core` process-wide state, the registry of
/// live per-account sessions (openspec/changes/multi-user-isolation), and the session-id store
/// `crates/core` deliberately doesn't own (design.md's "Enforcement" decision). `Clone` is cheap —
/// every field is already `Arc`-backed internally.
#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) shared: Arc<Shared>,
    pub(crate) accounts: Arc<session::AccountRegistry>,
    pub(crate) sessions: Arc<session::SessionStore>,
}

pub(crate) type ApiError = (StatusCode, String);

/// Shared by every route: the anonymous metadata client every `innertube` call in this server
/// uses. Factored out once it's needed by more than the original single route.
pub(crate) fn metadata_client(state: &Shared) -> Result<&innertube::YouTubeClient, ApiError> {
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

/// The full route table, merged and bound to `state` — factored out of `main` so
/// `#[cfg(test)]` integration tests can drive it directly (`tower::ServiceExt::oneshot`) without
/// a bound socket. No `CorsLayer` here: that's a browser-only concern `main` adds on top.
fn app(state: ServerState) -> axum::Router {
    search::router()
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

    // No session key to restore at boot: `LastfmHandle` is one scrobbler task shared by every
    // concurrently signed-in account (openspec/changes/multi-user-isolation), while
    // `lastfm_session_key` is stored per-account — there is no single "the" account's key to
    // restore automatically any more. A user whose Last.fm was connected before a restart
    // reconnects via the titlebar button, same as a fresh sign-in would need to anyway.
    let lastfm = limusic_core::lastfm::spawn(None);

    // Same "cargo run development artifact" stance as the sqlite path above — a fixed relative
    // directory, not yet a real configuration story (http-search-vertical-slice design.md).
    let app_data_dir = std::path::Path::new("limusic-server-data");
    std::fs::create_dir_all(app_data_dir).expect("failed to create limusic-server-data");

    let shared = Arc::new(Shared::new(Clients::bundled(), db, lastfm, app_data_dir));
    let state = ServerState {
        shared,
        accounts: Arc::new(session::AccountRegistry::default()),
        sessions: Arc::new(session::SessionStore::default()),
    };

    // Permissive but explicit, not `CorsLayer::permissive()`: a named origin is one line more and
    // doesn't invite copy-pasting a wildcard into a later, less throwaway version of this file.
    // `allow_credentials` is needed now that routes set/read a session cookie.
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5183".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers([CONTENT_TYPE])
        .allow_credentials(true);

    let app = app(state).layer(cors);

    // Defaults to the existing localhost-only dev behavior; a container deployment sets
    // BIND_ADDR=0.0.0.0:8787 so Docker's port publishing (itself bound to 127.0.0.1 on the host,
    // see deploy/docker-compose.yml) can reach the process from outside its network namespace.
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_owned());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    tracing::info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
