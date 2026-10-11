// Library routes (openspec/changes/signed-in-http-parity, tasks 3.1-3.3).
use axum::{extract::State, routing::get, Json, Router};
use innertube::{BrowseItem, HistoryGroup};

use crate::session::RequireSession;
use crate::{command_err, ApiError, ServerState};

async fn get_library(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Result<Json<Vec<BrowseItem>>, ApiError> {
    Ok(Json(limusic_core::commands::library::get_library(&state.app).await.map_err(command_err)?))
}

async fn get_library_albums(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Result<Json<Vec<BrowseItem>>, ApiError> {
    Ok(Json(
        limusic_core::commands::library::get_library_albums(&state.app)
            .await
            .map_err(command_err)?,
    ))
}

async fn get_library_artists(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Result<Json<Vec<BrowseItem>>, ApiError> {
    Ok(Json(
        limusic_core::commands::library::get_library_artists(&state.app)
            .await
            .map_err(command_err)?,
    ))
}

async fn get_upload_albums(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Result<Json<Vec<BrowseItem>>, ApiError> {
    Ok(Json(
        limusic_core::commands::library::get_upload_albums(&state.app)
            .await
            .map_err(command_err)?,
    ))
}

async fn get_history(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Result<Json<Vec<HistoryGroup>>, ApiError> {
    Ok(Json(limusic_core::commands::library::get_history(&state.app).await.map_err(command_err)?))
}

async fn get_play_counts(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Json<std::collections::HashMap<String, i64>> {
    Json(limusic_core::commands::library::play_counts(&state.app))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/library", get(get_library))
        .route("/api/library/albums", get(get_library_albums))
        .route("/api/library/artists", get(get_library_artists))
        .route("/api/library/uploads", get(get_upload_albums))
        .route("/api/history", get(get_history))
        .route("/api/play-counts", get(get_play_counts))
}
