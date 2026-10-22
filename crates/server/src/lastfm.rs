// Last.fm routes (openspec/changes/signed-in-http-parity, task 8.1).
use axum::{extract::State, routing::post, Json, Router};

use crate::session::RequireSession;
use crate::{command_err, ApiError, ServerState};

#[derive(serde::Serialize)]
struct ConnectResponse {
    #[serde(rename = "authorizeUrl")]
    authorize_url: String,
}

async fn lastfm_connect(
    State(state): State<ServerState>,
    session: RequireSession,
) -> Result<Json<ConnectResponse>, ApiError> {
    let owned = session.scoped_owned(&state);
    let authorize_url =
        limusic_core::commands::lastfm::lastfm_connect(owned).await.map_err(command_err)?;
    Ok(Json(ConnectResponse { authorize_url }))
}

async fn lastfm_disconnect(State(state): State<ServerState>, session: RequireSession) {
    let scoped = session.scoped(&state);
    limusic_core::commands::lastfm::lastfm_disconnect(&scoped);
}

async fn lastfm_status(
    State(state): State<ServerState>,
    session: RequireSession,
) -> Json<serde_json::Value> {
    let scoped = session.scoped(&state);
    Json(limusic_core::commands::lastfm::lastfm_status(&scoped))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/lastfm/connect", post(lastfm_connect))
        .route("/api/lastfm/disconnect", post(lastfm_disconnect))
        .route("/api/lastfm/status", axum::routing::get(lastfm_status))
}
