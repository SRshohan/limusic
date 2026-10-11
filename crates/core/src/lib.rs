//! Tauri-free account/library/settings logic, forked from `src-tauri` (openspec/changes/
//! signed-in-http-parity). See that change's design.md for what was forked, what was trimmed, and
//! why — this is not a verbatim copy of `src-tauri`'s equivalent modules.

pub mod blocked;
pub mod cipher;
pub mod commands;
pub mod db;
pub mod events;
pub mod http;
pub mod lastfm;
pub mod lyrics;
pub mod media;
pub mod potoken;
pub mod romanize;
pub mod state;
pub(crate) mod v8root;
