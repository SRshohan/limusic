//! Server-owned session store + the `RequireSession` extractor (openspec/changes/
//! signed-in-http-parity, tasks 2.1/2.3). Deliberately not in `crates/core`: HTTP sessions are a
//! transport-boundary concept the Tauri-mirroring fork has never needed — see design.md's
//! "Enforcement" decision.
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use rand::RngCore;

use crate::ApiError;

const SESSION_COOKIE: &str = "limusic_session";

/// Maps an opaque, server-generated session id to the account it was issued for. The YouTube
/// session cookie itself is never used as this id, and never stored here — see design.md's "Auth"
/// decision. Single-account-per-instance (design.md Non-Goals): every entry refers to whichever
/// one account this server process is signed into: a session id is a *proof of having signed in*,
/// not a selector between several concurrently-active accounts.
#[derive(Default)]
pub(crate) struct SessionStore {
    // session id -> account_key (the StoredAccount this session was issued for).
    sessions: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl SessionStore {
    /// Mint a new session id for `account_key`, distinct from the YouTube cookie that
    /// authenticated it.
    pub(crate) fn create(&self, account_key: String) -> String {
        let id = generate_id();
        self.sessions.lock().unwrap().insert(id.clone(), account_key);
        id
    }

    pub(crate) fn account_key_for(&self, session_id: &str) -> Option<String> {
        self.sessions.lock().unwrap().get(session_id).cloned()
    }

    /// Every session belongs to the one account a sign-out just ended, so all of them go.
    pub(crate) fn clear(&self) {
        self.sessions.lock().unwrap().clear();
    }
}

fn generate_id() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// An axum extractor gating a handler behind a valid session: resolves the request's session
/// cookie to a stored account, or rejects with a clear client error — before the handler's own
/// logic ever runs. See design.md's "Enforcement" decision for why this lives here, not in
/// `crates/core`.
pub(crate) struct RequireSession {
    pub(crate) account_key: String,
}

impl FromRequestParts<crate::ServerState> for RequireSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::ServerState,
    ) -> Result<Self, Self::Rejection> {
        let unauthorized = || {
            (StatusCode::UNAUTHORIZED, "sign in first — no valid session cookie".to_owned())
        };
        let cookie_header = parts
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(unauthorized)?;
        let session_id = cookie_header
            .split(';')
            .map(str::trim)
            .find_map(|kv| kv.strip_prefix(&format!("{SESSION_COOKIE}=")))
            .ok_or_else(unauthorized)?;
        let account_key =
            state.sessions.account_key_for(session_id).ok_or_else(unauthorized)?;
        // Single-account-per-instance: a session minted for an account this server is no longer
        // signed into (a sign-out, or signing into a different account since) is stale.
        if !state.app.it.is_logged_in() {
            return Err(unauthorized());
        }
        Ok(RequireSession { account_key })
    }
}

/// Build the `Set-Cookie` header value for a freshly minted session id.
pub(crate) fn session_cookie_header(session_id: &str) -> String {
    // `HttpOnly`: never readable from page JS (design.md's "Auth" decision). `SameSite=Lax`: sent
    // on normal navigation/top-level requests, not on a cross-site POST — reasonable default for a
    // same-site SPA talking to its own API origin. No `Secure` yet: this is a plain-`http://`
    // localhost dev server, same as the rest of this slice (see http-search-vertical-slice's own
    // Non-Goals on hosting/TLS).
    format!("{SESSION_COOKIE}={session_id}; HttpOnly; SameSite=Lax; Path=/")
}

/// The `Set-Cookie` value that clears the session cookie (sign-out).
pub(crate) fn clear_session_cookie_header() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::SessionStore;

    #[test]
    fn a_session_id_resolves_to_its_account_and_is_never_the_cookie_itself() {
        let store = SessionStore::default();
        let cookie = "SAPISID=super-secret-sapisid-value; SID=abc";
        let account_key = limusic_core::db::account_key(cookie).unwrap();

        let session_id = store.create(account_key.clone());

        assert_eq!(store.account_key_for(&session_id), Some(account_key));
        assert_ne!(session_id, cookie, "the session id must never be the YouTube cookie itself");
        assert!(
            !session_id.contains("super-secret-sapisid-value"),
            "the session id must not embed the raw cookie value either"
        );
    }

    #[test]
    fn an_unknown_session_id_resolves_to_nothing() {
        let store = SessionStore::default();
        assert_eq!(store.account_key_for("not-a-real-session-id"), None);
    }

    #[test]
    fn clear_invalidates_every_session() {
        let store = SessionStore::default();
        let a = store.create("account-a".to_owned());
        let b = store.create("account-b".to_owned());
        store.clear();
        assert_eq!(store.account_key_for(&a), None);
        assert_eq!(store.account_key_for(&b), None);
    }
}
