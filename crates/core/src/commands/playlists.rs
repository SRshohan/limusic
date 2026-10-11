//! Playlist write-action commands, ported from `src-tauri/src/commands.rs`'s "write actions"
//! section. Adapted, not verbatim:
//! - Every command drops its "local playlist" branch (`LOCALPLAYLIST:` ids) — `local.rs`-adjacent,
//!   not forked (see design.md). `editable_playlist` already rejects a `LOCALPLAYLIST:`/`ON_REPEAT`
//!   id with a clear error before any `innertube` call, same as the desktop version.
//! - Every command drops the `playlist_track` SQLite index bookkeeping
//!   (`add_playlist_track`/`remove_playlist_track`/`forget_playlist`): its only reader,
//!   `playlist_memberships`, was already dropped from `db.rs` (dead code serving
//!   `playlist_index`/`sync_playlist_index`, out of scope — see `db.rs`'s note).
//! - `set_playlist_cover` is redesigned for HTTP: the desktop version copies a path from the local
//!   filesystem into its own covers directory and serves it back over Tauri's asset protocol. An
//!   HTTP caller has no "local path on the server" to hand over — it has image bytes (a browser
//!   `File`/`Blob`) — so this version takes those bytes directly and uploads straight to YouTube,
//!   with no server-side copy or covers directory at all.
use crate::state::{is_local_playlist, AppState, ON_REPEAT_ID};

const LIKED_MUSIC_ID: &str = "VLLM";

fn metadata_client(state: &AppState) -> Result<&innertube::YouTubeClient, String> {
    state.clients.get(innertube::METADATA_CLIENT).ok_or_else(|| "metadata client missing".into())
}

fn editable_playlist<'a>(
    state: &'a AppState,
    playlist_id: &str,
) -> Result<&'a innertube::YouTubeClient, String> {
    if playlist_id == ON_REPEAT_ID {
        return Err("On Repeat builds itself from what you play.".into());
    }
    if playlist_id == LIKED_MUSIC_ID {
        return Err("Liked Music follows your likes; like the song instead.".into());
    }
    if is_local_playlist(playlist_id) {
        return Err("This playlist is on this device, so that isn't available for it.".into());
    }
    metadata_client(state)
}

/// Answers the new playlist's id. No `local` option here (desktop-only, see module doc).
pub async fn create_playlist(state: &AppState, title: &str) -> Result<String, String> {
    let client = metadata_client(state)?;
    state.it.create_playlist(client, title).await.map_err(|e| e.to_string())
}

pub async fn delete_playlist(state: &AppState, playlist_id: &str) -> Result<(), String> {
    let client = editable_playlist(state, playlist_id)?;
    state.it.delete_playlist(client, playlist_id).await.map_err(|e| e.to_string())
}

/// Each field is `None` when the caller left it alone, and only what changed is sent.
pub async fn edit_playlist_details(
    state: &AppState,
    playlist_id: &str,
    name: Option<&str>,
    description: Option<&str>,
    public: Option<bool>,
) -> Result<(), String> {
    let client = editable_playlist(state, playlist_id)?;
    // The switch is two-state; YouTube's third value (UNLISTED) is only ever left as it was.
    let privacy = public.map(|p| if p { "PUBLIC" } else { "PRIVATE" });
    state
        .it
        .playlist_edit_details(client, playlist_id, name, description, privacy)
        .await
        .map_err(|e| e.to_string())
}

/// `None` means "a JPEG/PNG under 8 MB" wasn't recognized from the bytes' own magic number — the
/// HTTP route never had a filename extension to trust the way the desktop picker did.
fn sniff_image_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("png")
    } else {
        None
    }
}

const MAX_COVER_BYTES: usize = 8 * 1024 * 1024;

/// `image = None` clears the cover, answering the thumbnail YouTube rebuilt in its place.
/// `image = Some(bytes)` uploads it directly — no local copy, no covers directory (see module doc).
pub async fn set_playlist_cover(
    state: &AppState,
    playlist_id: &str,
    image: Option<Vec<u8>>,
) -> Result<Option<String>, String> {
    let Some(bytes) = image else {
        if playlist_id == ON_REPEAT_ID || is_local_playlist(playlist_id) || !state.it.is_logged_in()
        {
            return Ok(None);
        }
        let client = metadata_client(state)?;
        return state.it.playlist_clear_cover(client, playlist_id).await.map_err(|e| e.to_string());
    };
    if bytes.len() > MAX_COVER_BYTES {
        return Err("That image is over 8 MB. Pick a smaller one.".into());
    }
    if sniff_image_format(&bytes).is_none() {
        return Err("Pick a JPEG or PNG image: YouTube Music won't take anything else.".into());
    }
    let client = editable_playlist(state, playlist_id)?;
    state.it.playlist_set_cover(client, playlist_id, bytes).await.map_err(|e| e.to_string())?;
    Ok(None)
}

pub async fn add_to_playlist(
    state: &AppState,
    playlist_id: &str,
    video_id: &str,
    allow_duplicates: bool,
) -> Result<bool, String> {
    let client = editable_playlist(state, playlist_id)?;
    state.it.playlist_add(client, playlist_id, video_id, allow_duplicates).await.map_err(|e| e.to_string())
}

pub async fn remove_from_playlist(
    state: &AppState,
    playlist_id: &str,
    video_id: &str,
    set_video_id: &str,
) -> Result<(), String> {
    let client = editable_playlist(state, playlist_id)?;
    state.it.playlist_remove(client, playlist_id, video_id, set_video_id).await.map_err(|e| e.to_string())
}

/// All or nothing: YouTube applies the whole action list or rejects it, so the caller can revert
/// an optimistic removal on an error without working out which rows made it.
pub async fn remove_many_from_playlist(
    state: &AppState,
    playlist_id: &str,
    tracks: &[(String, String)],
) -> Result<(), String> {
    let client = editable_playlist(state, playlist_id)?;
    state.it.playlist_remove_many(client, playlist_id, tracks).await.map_err(|e| e.to_string())
}

/// Store a sort order on a playlist, so YouTube Music and every other client show it the same way.
/// Only for a playlist whose `sortMenu.editable` said the options are writes — elsewhere YouTube
/// answers this with an error.
pub async fn set_playlist_sort(
    state: &AppState,
    playlist_id: &str,
    sort: innertube::PlaylistSort,
) -> Result<(), String> {
    let client = editable_playlist(state, playlist_id)?;
    state.it.playlist_set_sort(client, playlist_id, sort).await.map_err(|e| e.to_string())
}
