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
    _session: RequireSession,
) -> Result<Json<ConnectResponse>, ApiError> {
    let authorize_url =
        limusic_core::commands::lastfm::lastfm_connect(state.app.clone()).await.map_err(command_err)?;
    Ok(Json(ConnectResponse { authorize_url }))
}

async fn lastfm_disconnect(State(state): State<ServerState>, _session: RequireSession) {
    limusic_core::commands::lastfm::lastfm_disconnect(&state.app);
}

async fn lastfm_status(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Json<serde_json::Value> {
    Json(limusic_core::commands::lastfm::lastfm_status(&state.app))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/lastfm/connect", post(lastfm_connect))
        .route("/api/lastfm/disconnect", post(lastfm_disconnect))
        .route("/api/lastfm/status", axum::routing::get(lastfm_status))
}
