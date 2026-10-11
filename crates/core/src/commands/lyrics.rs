//! Lyrics commands, ported from `src-tauri/src/commands.rs`'s "lyrics" section — thin wrappers
//! over `crate::lyrics`, unchanged.
use crate::lyrics::{Lyrics, LyricsRequest, ProviderInfo};
use crate::state::AppState;

#[allow(clippy::too_many_arguments)]
pub async fn get_lyrics(
    state: &AppState,
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    duration: Option<f64>,
    source: Option<String>,
) -> Result<Option<Lyrics>, String> {
    let req = LyricsRequest { video_id, title, artists, album, duration };
    crate::lyrics::get_lyrics(state, req, source).await
}

#[allow(clippy::too_many_arguments)]
pub async fn choose_lyrics_source(
    state: &AppState,
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    duration: Option<f64>,
    source: Option<String>,
) -> Option<Lyrics> {
    let req = LyricsRequest { video_id, title, artists, album, duration };
    crate::lyrics::choose_source(state, req, source).await
}

pub fn set_lyrics_offset(state: &AppState, video_id: &str, offset_ms: i64) {
    crate::lyrics::set_offset(state, video_id, offset_ms);
}

pub fn lyrics_providers(state: &AppState) -> Vec<ProviderInfo> {
    crate::lyrics::providers(state)
}
