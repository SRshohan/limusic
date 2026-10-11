//! Library commands, ported from `src-tauri/src/commands.rs`'s "browse / library" + "write
//! actions" sections. Adapted, not verbatim (see each function's note):
//! - `get_library` drops the local-playlists splice and custom-cover lookup (`local.rs`-adjacent,
//!   desktop filesystem only — see design.md). On Repeat stays: it degrades to empty naturally
//!   (nothing records a play server-side yet, same gap `signed-in-http-parity`'s design.md's
//!   task 3.3 flagged), not an error, and costs nothing to leave in for when something does.
use innertube::{BrowseItem, HistoryGroup};

use crate::state::{AppState, ON_REPEAT_ID, ON_REPEAT_LIMIT, ON_REPEAT_WINDOW_SECS};

fn metadata_client(state: &AppState) -> Result<&innertube::YouTubeClient, String> {
    state.clients.get(innertube::METADATA_CLIENT).ok_or_else(|| "metadata client missing".into())
}

/// The On Repeat track list: most-played first, over the trailing window. Rows whose stored JSON
/// no longer parses (a `SongItem` shape change) are dropped rather than failing the whole page.
fn on_repeat_songs(state: &AppState) -> Vec<innertube::SongItem> {
    let since = crate::db::now_secs() - ON_REPEAT_WINDOW_SECS;
    state.db.top_plays(since, ON_REPEAT_LIMIT).into_iter().filter_map(|(json, _plays)| serde_json::from_str(&json).ok()).collect()
}

pub async fn get_library(state: &AppState) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(state)?;
    // Signed out there is no YouTube library to ask for (the browse would come back as a sign-in
    // shell), but On Repeat is built from this server's play history and is still real.
    let mut items = if state.it.is_logged_in() {
        state.it.library_playlists(client).await.map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let songs = on_repeat_songs(state);
    if !songs.is_empty() {
        items.insert(
            0,
            BrowseItem {
                kind: "playlist",
                id: ON_REPEAT_ID.into(),
                title: "On Repeat".into(),
                subtitle: Some(format!("{} songs", songs.len())),
                thumbnail: None,
                duration: None,
                album_id: None,
                artist_runs: Vec::new(),
                play_count: None,
                is_video: false,
                is_upload: false,
                explicit: false,
            },
        );
    }
    Ok(items)
}

/// Empty rather than an error when signed out: the Library page merges the user's local saves into
/// these grids, so "nothing of yours on YouTube" is an answer, not a failure.
pub async fn get_library_albums(state: &AppState) -> Result<Vec<BrowseItem>, String> {
    if !state.it.is_logged_in() {
        return Ok(Vec::new());
    }
    let client = metadata_client(state)?;
    state.it.library_albums(client).await.map_err(|e| e.to_string())
}

/// The user's own uploaded albums (Library ▸ Uploads ▸ Albums).
pub async fn get_upload_albums(state: &AppState) -> Result<Vec<BrowseItem>, String> {
    if !state.it.is_logged_in() {
        return Ok(Vec::new());
    }
    let client = metadata_client(state)?;
    state.it.upload_albums(client).await.map_err(|e| e.to_string())
}

/// The account's YouTube Music play history, grouped by day. Empty when signed out, same as the
/// library grids: history lives on the account, and there is nothing to fail about not having one.
pub async fn get_history(state: &AppState) -> Result<Vec<HistoryGroup>, String> {
    if !state.it.is_logged_in() {
        return Ok(Vec::new());
    }
    let client = metadata_client(state)?;
    state.it.history(client).await.map_err(|e| e.to_string())
}

pub async fn get_library_artists(state: &AppState) -> Result<Vec<BrowseItem>, String> {
    if !state.it.is_logged_in() {
        return Ok(Vec::new());
    }
    let client = metadata_client(state)?;
    state.it.library_artists(client).await.map_err(|e| e.to_string())
}

/// videoId → how many times it was played, over the same trailing window On Repeat is built from.
/// Always empty until something records a play server-side (see `on_repeat_songs`'s note).
pub fn play_counts(state: &AppState) -> std::collections::HashMap<String, i64> {
    state.db.play_counts(crate::db::now_secs() - ON_REPEAT_WINDOW_SECS).into_iter().collect()
}
