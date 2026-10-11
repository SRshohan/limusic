//! Rating/save/subscribe commands, ported from `src-tauri/src/commands.rs`'s "write actions"
//! section. `rate_epoch`'s bump is dropped (see design.md: it only ever guarded a UI-optimistic
//! `refresh_rating`, which read a `queue` that doesn't exist here).
use innertube::Rating;

use crate::state::AppState;

fn metadata_client(state: &AppState) -> Result<&innertube::YouTubeClient, String> {
    state.clients.get(innertube::METADATA_CLIENT).ok_or_else(|| "metadata client missing".into())
}

/// Like, dislike, or clear a track's rating. One call for all three: YouTube's states are
/// mutually exclusive, so a dislike un-likes in the same call.
pub async fn rate(state: &AppState, video_id: &str, rating: Rating) -> Result<(), String> {
    let client = metadata_client(state)?;
    state.it.rate(client, video_id, rating).await.map_err(|e| e.to_string())
}

/// Add a song to the library (Library ▸ Songs), or take it out. `token` comes from the row's own
/// menu (`SongItem.library`) — a feedback action, not a rating, so it leaves Liked Music alone.
pub async fn set_song_saved(state: &AppState, token: &str) -> Result<(), String> {
    let client = metadata_client(state)?;
    state.it.feedback(client, token).await.map_err(|e| e.to_string())
}

/// Save an album to the library, or remove it. `playlist_id` is the album's `OLAK5uy_…`
/// (`AlbumPage.playlistId`).
pub async fn set_album_saved(
    state: &AppState,
    playlist_id: &str,
    saved: bool,
) -> Result<(), String> {
    let client = metadata_client(state)?;
    state.it.like_playlist(client, playlist_id, saved).await.map_err(|e| e.to_string())
}

pub async fn subscribe(
    state: &AppState,
    channel_id: &str,
    subscribed: bool,
) -> Result<(), String> {
    let client = metadata_client(state)?;
    state.it.subscribe(client, channel_id, subscribed).await.map_err(|e| e.to_string())
}
