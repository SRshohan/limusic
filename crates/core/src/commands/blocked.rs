//! Blocked-artists commands, ported from `src-tauri/src/commands.rs`'s "blocked artists" section.
//! Drops the live-queue purge (`state.purge_blocked`) — playback-queue bookkeeping, dropped along
//! with `queue` itself (see design.md). The fetch-filter side (`it.set_blocked`) is kept: it still
//! governs what any future request returns, which is the half of this that matters without a
//! queue to also clean out.
//!
//! Global, not per-account (openspec/changes/multi-user-isolation design.md Decision 2/5): these
//! routes take no session today, by a prior, deliberate decision (see `http-blocked-artists`'s own
//! spec) this change doesn't revisit. The filter is applied to `shared.anonymous_it` — the one
//! `InnerTube` every anonymous route already shares; a signed-in `AccountSession`'s own `it` isn't
//! reached by it, a narrower limitation than today's single-shared-`it` world had, but not a
//! regression this change's scope covers fixing.
use crate::blocked::BlockedArtist;
use crate::state::Shared;

pub fn get_blocked_artists(shared: &Shared) -> Vec<BlockedArtist> {
    crate::blocked::list(&shared.db)
}

/// Block an artist: persist, re-arm the fetch filter. Returns the new list. `id` is the channel
/// browseId when the caller has one (a track row often does not).
pub fn block_artist(
    shared: &Shared,
    id: Option<String>,
    name: &str,
) -> Result<Vec<BlockedArtist>, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("an artist needs a name to be blocked".into());
    }
    let list =
        crate::blocked::block(&shared.db, BlockedArtist { id: id.filter(|i| !i.is_empty()), name });
    shared.anonymous_it.set_blocked(crate::blocked::block_list(&shared.db));
    Ok(list)
}

pub fn unblock_artist(shared: &Shared, key: &str) -> Vec<BlockedArtist> {
    let list = crate::blocked::unblock(&shared.db, key);
    shared.anonymous_it.set_blocked(crate::blocked::block_list(&shared.db));
    list
}
