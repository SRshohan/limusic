//! Last.fm commands, ported from `src-tauri/src/commands.rs`'s "Last.fm scrobbling" section.
//! `lastfm_connect` now returns the authorize URL instead of `()` — see `crate::lastfm::connect`'s
//! doc comment for why (the server has no local browser to open on the caller's behalf).
use crate::state::{Scoped, ScopedOwned};

/// Owned: the connect flow spawns a background poll outliving this call.
pub async fn lastfm_connect(state: ScopedOwned) -> Result<String, String> {
    crate::lastfm::connect(state).await
}

pub fn lastfm_disconnect(state: &Scoped<'_>) {
    crate::lastfm::disconnect(state);
}

/// `{ connected, username }` from the persisted session.
pub fn lastfm_status(state: &Scoped<'_>) -> serde_json::Value {
    crate::lastfm::status(state)
}
