// Rating/save/subscribe routes (openspec/changes/signed-in-http-parity, tasks 5.1-5.3).
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Json, Router,
};
use innertube::Rating;
use serde::Deserialize;

use crate::session::RequireSession;
use crate::{command_err, ApiError, ServerState};

#[derive(Deserialize)]
struct RatingBody {
    rating: Rating,
}

async fn rate(
    State(state): State<ServerState>,
    session: RequireSession,
    Path(video_id): Path<String>,
    Json(body): Json<RatingBody>,
) -> Result<StatusCode, ApiError> {
    let scoped = session.scoped(&state);
    limusic_core::commands::ratings::rate(&scoped, &video_id, body.rating)
        .await
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SavedBody {
    saved: bool,
}

/// `token` comes from the song row's own `library` field — the caller already has it from a prior
/// browse/search response, so it's the request body, not something this route looks up itself.
#[derive(Deserialize)]
struct SongSavedBody {
    token: String,
}

async fn set_song_saved(
    State(state): State<ServerState>,
    session: RequireSession,
    Json(body): Json<SongSavedBody>,
) -> Result<StatusCode, ApiError> {
    let scoped = session.scoped(&state);
    limusic_core::commands::ratings::set_song_saved(&scoped, &body.token)
        .await
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_album_saved(
    State(state): State<ServerState>,
    session: RequireSession,
    Path(playlist_id): Path<String>,
    Json(body): Json<SavedBody>,
) -> Result<StatusCode, ApiError> {
    let scoped = session.scoped(&state);
    limusic_core::commands::ratings::set_album_saved(&scoped, &playlist_id, body.saved)
        .await
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SubscribedBody {
    subscribed: bool,
}

async fn subscribe(
    State(state): State<ServerState>,
    session: RequireSession,
    Path(channel_id): Path<String>,
    Json(body): Json<SubscribedBody>,
) -> Result<StatusCode, ApiError> {
    let scoped = session.scoped(&state);
    limusic_core::commands::ratings::subscribe(&scoped, &channel_id, body.subscribed)
        .await
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/songs/{video_id}/rating", put(rate))
        .route("/api/songs/saved", put(set_song_saved))
        .route("/api/albums/{playlist_id}/saved", put(set_album_saved))
        .route("/api/artists/{channel_id}/subscribed", put(subscribe))
}
