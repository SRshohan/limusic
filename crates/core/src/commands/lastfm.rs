//! Last.fm commands, ported from `src-tauri/src/commands.rs`'s "Last.fm scrobbling" section.
//! `lastfm_connect` now returns the authorize URL instead of `()` — see `crate::lastfm::connect`'s
//! doc comment for why (the server has no local browser to open on the caller's behalf).
use std::sync::Arc;

use crate::state::AppState;

pub async fn lastfm_connect(state: Arc<AppState>) -> Result<String, String> {
    crate::lastfm::connect(state).await
}

pub fn lastfm_disconnect(state: &AppState) {
    crate::lastfm::disconnect(state);
}

/// `{ connected, username }` from the persisted session.
pub fn lastfm_status(state: &AppState) -> serde_json::Value {
    crate::lastfm::status(state)
}
