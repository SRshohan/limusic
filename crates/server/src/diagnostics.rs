// Diagnostics routes (openspec/changes/signed-in-http-parity, task 11.1). Fresh code, not forked
// from `src-tauri/src/diagnostics.rs` (desktop log-file/install-kind logic that doesn't transfer —
// see design.md). No log file exists to tail here: this process logs to stdout via
// `tracing_subscriber`, not a file, so there's nothing for "diagnostics" to read back except the
// environment header the desktop version also leads with.
use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use crate::ServerState;

/// Env vars worth knowing about in a report, much shorter than the desktop's `ENV_KEYS` (no
/// windowing/compositor/webview concerns here). Reported as `set`/`unset`, never by value — same
/// secrecy convention `src-tauri/src/diagnostics.rs` uses for anything that can carry a credential.
const ENV_KEYS: &[&str] = &["RUST_LOG", "LIMUSIC_LASTFM_API_KEY", "LIMUSIC_LASTFM_API_SECRET"];

fn header_text(state: &ServerState) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "limusic-server {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let _ = writeln!(out, "signed in: {}", state.app.it.is_logged_in());
    out.push_str("env:\n");
    for key in ENV_KEYS {
        let present = std::env::var(key).is_ok_and(|v| !v.is_empty());
        let _ = writeln!(out, "  {key}: {}", if present { "set" } else { "unset" });
    }
    out
}

async fn diagnostics(State(state): State<ServerState>) -> String {
    header_text(&state)
}

/// Same content as `diagnostics` — there's no separate log tail to trim down to a summary here,
/// unlike the desktop version (whose "summary" is the header alone, without the log).
async fn diagnostics_summary(State(state): State<ServerState>) -> String {
    header_text(&state)
}

async fn save_diagnostics(State(state): State<ServerState>) -> Response {
    let body = header_text(&state);
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"limusic-diagnostics.txt\""),
        ],
        body,
    )
        .into_response()
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/diagnostics/summary", get(diagnostics_summary))
        .route("/api/diagnostics/save", get(save_diagnostics))
}
