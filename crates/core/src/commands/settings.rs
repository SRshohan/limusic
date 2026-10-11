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
use crate::state::AppState;

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

pub fn get_setting(state: &AppState, key: &str) -> Result<Option<String>, String> {
    if !SETTINGS_KEYS.contains(&key) {
        return Err(format!("unknown setting: {key}"));
    }
    Ok(state.db.get_setting(key))
}

/// All settings in one call — matches the desktop `get_settings` command's bulk shape, over this
/// server's smaller key set. Unset keys are simply absent, not `null` entries.
pub fn get_all_settings(state: &AppState) -> std::collections::HashMap<String, String> {
    SETTINGS_KEYS
        .iter()
        .filter_map(|&key| state.db.get_setting(key).map(|value| (key.to_owned(), value)))
        .collect()
}

pub fn set_setting(state: &AppState, key: &str, value: &str) -> Result<(), String> {
    if !SETTINGS_KEYS.contains(&key) {
        return Err(format!("unknown setting: {key}"));
    }
    state.db.set_setting(key, value);
    if key == "locale" {
        state.it.set_locale(value);
    }
    if key == "hide_videos" {
        state.it.set_hide_videos(value == "true");
    }
    if key == "lyrics_providers" {
        state.db.clear_lyrics_cache();
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
/// wipe here (`cache_dir`, dropped — see design.md).
pub fn clear_caches(state: &AppState) {
    state.db.clear_stream_cache();
    state.db.delete_setting("potoken_session");
}
