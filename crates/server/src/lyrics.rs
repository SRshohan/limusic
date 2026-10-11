// Lyrics routes (openspec/changes/signed-in-http-parity, tasks 7.1-7.3). Anonymous — lyrics have
// no sign-in requirement in the original Tauri commands either.
use axum::{
    extract::{Path, Query, State},
    routing::{get, put},
    Json, Router,
};
use limusic_core::lyrics::{Lyrics, ProviderInfo};
use serde::Deserialize;

use crate::{command_err, ApiError, ServerState};

#[derive(Deserialize)]
struct LyricsParams {
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    duration: Option<f64>,
}

async fn get_lyrics(
    State(state): State<ServerState>,
    Query(params): Query<LyricsParams>,
) -> Result<Json<Option<Lyrics>>, ApiError> {
    Ok(Json(
        limusic_core::commands::lyrics::get_lyrics(
            &state.app,
            params.video_id,
            params.title,
            params.artists,
            params.album,
            params.duration,
            None,
        )
        .await
        .map_err(command_err)?,
    ))
}

#[derive(Deserialize)]
struct ChooseSourceParams {
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    duration: Option<f64>,
    source: Option<String>,
}

async fn choose_lyrics_source(
    State(state): State<ServerState>,
    Query(params): Query<ChooseSourceParams>,
) -> Json<Option<Lyrics>> {
    Json(
        limusic_core::commands::lyrics::choose_lyrics_source(
            &state.app,
            params.video_id,
            params.title,
            params.artists,
            params.album,
            params.duration,
            params.source,
        )
        .await,
    )
}

#[derive(Deserialize)]
struct OffsetBody {
    #[serde(rename = "offsetMs")]
    offset_ms: i64,
}

async fn set_lyrics_offset(
    State(state): State<ServerState>,
    Path(video_id): Path<String>,
    Json(body): Json<OffsetBody>,
) {
    limusic_core::commands::lyrics::set_lyrics_offset(&state.app, &video_id, body.offset_ms);
}

async fn lyrics_providers(State(state): State<ServerState>) -> Json<Vec<ProviderInfo>> {
    Json(limusic_core::commands::lyrics::lyrics_providers(&state.app))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/lyrics", get(get_lyrics))
        .route("/api/lyrics/choose-source", get(choose_lyrics_source))
        .route("/api/lyrics/{video_id}/offset", put(set_lyrics_offset))
        .route("/api/lyrics/providers", get(lyrics_providers))
}
