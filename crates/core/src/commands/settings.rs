//! Settings commands, ported from `src-tauri/src/commands.rs`'s `get_settings`/`set_setting`/
//! `get_stream_clients`/`clear_caches`. Heavily adapted, not verbatim: the desktop `set_setting`
//! is a ~10-way dispatcher (autostart registration, mpv gain/crossfade, Discord presence, ambient
//! WebGL, window decorations, …) — all playback/desktop-UI concerns this server doesn't have. Of
//! the desktop's 27-key `UI_SETTINGS` allowlist, only the keys with a real effect here are kept:
//! `locale` (the language `innertube` requests in), `hide_videos` (whether video results are
//! filtered), and `lyrics_providers` (changing it invalidates the lyrics cache). Everything else
//! in this list is a plain stored value with no side effect yet (`quality`, `enable_history`,
//! `disabled_stream_clients`, `lastfm_primary_artist`, `lastfm_primary_strict`) — kept because a
//! later change (streaming, or wiring `record_history`) will want them, not because they do
//! anything today. `volume`/`proxy`/`discord_rpc`/`autostart`/`crossfade`/`system_titlebar`/etc.
//! are desktop-only and not in this list at all.
//!
//! openspec/changes/multi-user-isolation: every key here is per-account *except*
//! `lyrics_providers`, which stays the one global row it already was — `GET /api/lyrics` reads it
//! directly and is itself an anonymous, ungated route, so there's no session to scope it by (see
//! design.md Decision 2).
use crate::state::{Scoped, ANONYMOUS};

const SETTINGS_KEYS: [&str; 8] = [
    "quality",
    "enable_history",
    "disabled_stream_clients",
    "hide_videos",
    "lyrics_providers",
    "lastfm_primary_artist",
    "lastfm_primary_strict",
    "locale",
];

fn scope_for<'a>(state: &'a Scoped<'_>, key: &str) -> &'a str {
    if key == "lyrics_providers" { ANONYMOUS } else { state.account_key() }
}

pub fn get_setting(state: &Scoped<'_>, key: &str) -> Result<Option<String>, String> {
    if !SETTINGS_KEYS.contains(&key) {
        return Err(format!("unknown setting: {key}"));
    }
    Ok(state.db().get_setting(scope_for(state, key), key))
}

/// All settings in one call — matches the desktop `get_settings` command's bulk shape, over this
/// server's smaller key set. Unset keys are simply absent, not `null` entries.
pub fn get_all_settings(state: &Scoped<'_>) -> std::collections::HashMap<String, String> {
    SETTINGS_KEYS
        .iter()
        .filter_map(|&key| {
            state.db().get_setting(scope_for(state, key), key).map(|value| (key.to_owned(), value))
        })
        .collect()
}

pub fn set_setting(state: &Scoped<'_>, key: &str, value: &str) -> Result<(), String> {
    if !SETTINGS_KEYS.contains(&key) {
        return Err(format!("unknown setting: {key}"));
    }
    state.db().set_setting(scope_for(state, key), key, value);
    if key == "locale" {
        state.it().set_locale(value);
    }
    if key == "hide_videos" {
        state.it().set_hide_videos(value == "true");
    }
    if key == "lyrics_providers" {
        state.db().clear_lyrics_cache();
    }
    Ok(())
}

pub fn get_stream_clients() -> Vec<String> {
    let mut v = vec![innertube::MAIN_CLIENT.to_string()];
    v.extend(innertube::STREAM_FALLBACK_ORDER.iter().map(|s| s.to_string()));
    v
}

/// Clear the stream-URL cache and the stored PoToken session (same two things the desktop
/// `clear_caches` clears — it doesn't touch `lyrics_cache` either). No on-disk mpv audio cache to
/// wipe here (`cache_dir`, dropped — see design.md). `potoken_session` is global (see this file's
/// header), so this clears it for every account, not just the caller's — matches `potoken_session`
/// already being one shared BotGuard session, not a per-account one.
pub fn clear_caches(state: &Scoped<'_>) {
    state.db().clear_stream_cache();
    state.db().delete_setting(ANONYMOUS, "potoken_session");
}
