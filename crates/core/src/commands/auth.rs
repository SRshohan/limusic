//! Auth commands, ported from `src-tauri/src/commands.rs`'s "auth" + "saved Google accounts"
//! sections. `login_webview` is not ported (hidden-webview Google login — cookie-paste only, see
//! design.md); `get_playback` is not ported (playback).
use crate::state::{AccountSession, Scoped, Shared, SignInOutcome};

/// `None`: no session cookie at all (never signed in this browser) — trivially signed out, no
/// `AccountSession` to check. `Some`: delegate to that session's own snapshot.
pub async fn get_account(state: Option<&Scoped<'_>>) -> serde_json::Value {
    match state {
        Some(state) => state.account.account_snapshot(state.shared),
        None => serde_json::json!({ "signedIn": false }),
    }
}

/// The only sign-in entry point the web build has (cookie-paste) — no existing `AccountSession` to
/// scope by yet, since the account_key isn't known until the cookie is read. Returns the freshly
/// built session for the caller (`crates/server`) to register under its `account_key`.
pub async fn sign_in(
    shared: &Shared,
    cookie: String,
) -> Result<(AccountSession, SignInOutcome), String> {
    AccountSession::sign_in(shared, cookie, None).await
}

pub async fn get_account_identities(state: &Scoped<'_>) -> Result<Vec<serde_json::Value>, String> {
    state.account.account_identities(state.shared).await
}

pub async fn switch_account(
    state: &Scoped<'_>,
    selection_key: &str,
) -> Result<serde_json::Value, String> {
    state.account.switch_account(state.shared, selection_key).await
}

pub async fn sign_out(state: &Scoped<'_>) {
    state.account.sign_out(state.shared).await;
}

/// Saved Google accounts for the account menu — narrowed out of multi-user-isolation (see
/// `AccountSession::google_accounts`'s own doc comment).
pub async fn get_google_accounts(state: &Scoped<'_>) -> Vec<serde_json::Value> {
    state.account.google_accounts()
}

pub async fn switch_google_account(
    state: &Scoped<'_>,
    id: &str,
) -> Result<serde_json::Value, String> {
    state.account.switch_google_account(id).await
}

pub async fn remove_google_account(state: &Scoped<'_>, id: &str) {
    state.account.remove_google_account(id).await;
}
