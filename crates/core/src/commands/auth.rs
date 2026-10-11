//! Auth commands, ported from `src-tauri/src/commands.rs`'s "auth" + "saved Google accounts"
//! sections. `login_webview` is not ported (hidden-webview Google login — cookie-paste only, see
//! design.md); `get_playback` is not ported (playback).
use crate::state::AppState;

pub async fn get_account(state: &AppState) -> serde_json::Value {
    state.account_snapshot()
}

pub async fn get_account_identities(state: &AppState) -> Result<Vec<serde_json::Value>, String> {
    state.account_identities().await
}

pub async fn switch_account(
    state: &AppState,
    selection_key: &str,
) -> Result<serde_json::Value, String> {
    state.switch_account(selection_key).await
}

pub async fn sign_out(state: &AppState) {
    state.sign_out().await;
}

/// Saved Google accounts for the account menu. Display fields only — cookies, delegated ids and
/// visitorData never cross the HTTP boundary either.
pub async fn get_google_accounts(state: &AppState) -> Vec<serde_json::Value> {
    state.google_accounts()
}

/// Activate a saved Google account without a Google re-login. Fails if the stored cookie no
/// longer authenticates, in which case the caller should re-sign-in with that account.
pub async fn switch_google_account(
    state: &AppState,
    id: &str,
) -> Result<serde_json::Value, String> {
    state.switch_google_account(id).await
}

/// Delete a saved account. Removing the active one signs out; the others stay listed.
pub async fn remove_google_account(state: &AppState, id: &str) {
    state.remove_google_account(id).await;
}
