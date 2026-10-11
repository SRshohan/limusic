// Settings routes (openspec/changes/signed-in-http-parity, tasks 6.1-6.2).
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::session::RequireSession;
use crate::{command_err, ApiError, ServerState};

async fn get_all_settings(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Json<std::collections::HashMap<String, String>> {
    Json(limusic_core::commands::settings::get_all_settings(&state.app))
}

async fn get_setting(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(key): Path<String>,
) -> Result<Json<Option<String>>, ApiError> {
    Ok(Json(limusic_core::commands::settings::get_setting(&state.app, &key).map_err(command_err)?))
}

#[derive(Deserialize)]
struct ValueBody {
    value: String,
}

async fn set_setting(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(key): Path<String>,
    Json(body): Json<ValueBody>,
) -> Result<StatusCode, ApiError> {
    limusic_core::commands::settings::set_setting(&state.app, &key, &body.value)
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_stream_clients() -> Json<Vec<String>> {
    Json(limusic_core::commands::settings::get_stream_clients())
}

async fn clear_caches(State(state): State<ServerState>, _session: RequireSession) -> StatusCode {
    limusic_core::commands::settings::clear_caches(&state.app);
    StatusCode::NO_CONTENT
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/settings", get(get_all_settings))
        .route("/api/settings/stream-clients", get(get_stream_clients))
        .route("/api/settings/clear-caches", post(clear_caches))
        .route("/api/settings/{key}", get(get_setting).put(set_setting))
}
