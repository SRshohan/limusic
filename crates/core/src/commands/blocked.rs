//! Blocked-artists commands, ported from `src-tauri/src/commands.rs`'s "blocked artists" section.
//! Drops the live-queue purge (`state.purge_blocked`) — playback-queue bookkeeping, dropped along
//! with `queue` itself (see design.md). The fetch-filter side (`it.set_blocked`) is kept: it still
//! governs what any future request returns, which is the half of this that matters without a
//! queue to also clean out.
use crate::blocked::BlockedArtist;
use crate::state::AppState;

pub fn get_blocked_artists(state: &AppState) -> Vec<BlockedArtist> {
    crate::blocked::list(&state.db)
}

/// Block an artist: persist, re-arm the fetch filter. Returns the new list. `id` is the channel
/// browseId when the caller has one (a track row often does not).
pub fn block_artist(
    state: &AppState,
    id: Option<String>,
    name: &str,
) -> Result<Vec<BlockedArtist>, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("an artist needs a name to be blocked".into());
    }
    let list =
        crate::blocked::block(&state.db, BlockedArtist { id: id.filter(|i| !i.is_empty()), name });
    state.it.set_blocked(crate::blocked::block_list(&state.db));
    Ok(list)
}

pub fn unblock_artist(state: &AppState, key: &str) -> Vec<BlockedArtist> {
    let list = crate::blocked::unblock(&state.db, key);
    state.it.set_blocked(crate::blocked::block_list(&state.db));
    list
}
