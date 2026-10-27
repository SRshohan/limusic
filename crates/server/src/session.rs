//! Server-owned session store + the `RequireSession` extractor (openspec/changes/
//! signed-in-http-parity, tasks 2.1/2.3; openspec/changes/multi-user-isolation reworked both for
//! concurrent accounts). Deliberately not in `crates/core`: HTTP sessions are a transport-boundary
//! concept the Tauri-mirroring fork has never needed — see design.md's "Enforcement" decision.
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use limusic_core::state::AccountSession;
use rand::RngCore;

use crate::ApiError;

pub(crate) const SESSION_COOKIE: &str = "limusic_session";

/// Pull the opaque session id out of a request's `Cookie` header, or `None` if there isn't one.
/// Shared by `RequireSession` and by `auth::get_account`/`auth::sign_out`, which both need this
/// same lookup outside the extractor (one because it must still answer with no session at all,
/// the other because it needs the raw session id to remove, not just the account it names).
pub(crate) fn session_id_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{SESSION_COOKIE}=");
    cookie_header.split(';').map(str::trim).find_map(|kv| kv.strip_prefix(&prefix)).map(str::to_owned)
}

/// Maps an opaque, server-generated session id to the account it was issued for. The YouTube
/// session cookie itself is never used as this id, and never stored here — see design.md's "Auth"
/// decision. Several sessions can name several different, concurrently-live accounts
/// (openspec/changes/multi-user-isolation) — this map only resolves a session id to *which*
/// account it's for; `ServerState.accounts` holds the live `AccountSession`s themselves.
#[derive(Default)]
pub(crate) struct SessionStore {
    sessions: std::sync::Mutex<HashMap<String, String>>,
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

    /// End exactly one session (sign-out) — never every session, which would end every *other*
    /// concurrently signed-in account's session too (openspec/changes/multi-user-isolation).
    pub(crate) fn remove(&self, session_id: &str) {
        self.sessions.lock().unwrap().remove(session_id);
    }

    /// Whether any session still names `account_key` — the caller uses this to decide whether an
    /// `AccountSession` can be evicted from `ServerState.accounts` after a sign-out, or whether a
    /// different session is still using it.
    pub(crate) fn any_session_for(&self, account_key: &str) -> bool {
        self.sessions.lock().unwrap().values().any(|k| k == account_key)
    }
}

fn generate_id() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The live, concurrently-signed-in accounts (openspec/changes/multi-user-isolation) — keyed by
/// `account_key`, populated by a successful sign-in and looked up (never auto-created) by
/// `RequireSession`. A process restart loses this map's contents (in-memory only), which is why a
/// session minted before one is treated as stale rather than silently reconstructed — the same
/// "sign in again after a restart" behavior this server already had before this change.
#[derive(Default)]
pub(crate) struct AccountRegistry {
    accounts: RwLock<HashMap<String, Arc<AccountSession>>>,
}

impl AccountRegistry {
    pub(crate) fn get(&self, account_key: &str) -> Option<Arc<AccountSession>> {
        self.accounts.read().unwrap().get(account_key).cloned()
    }

    /// Register a freshly signed-in session, replacing any previous one for the same account_key
    /// (a re-sign-in refreshes the live session rather than leaving a stale one behind).
    pub(crate) fn insert(&self, session: AccountSession) -> Arc<AccountSession> {
        let session = Arc::new(session);
        self.accounts.write().unwrap().insert(session.account_key.clone(), session.clone());
        session
    }

    pub(crate) fn remove(&self, account_key: &str) {
        self.accounts.write().unwrap().remove(account_key);
    }

    pub(crate) fn len(&self) -> usize {
        self.accounts.read().unwrap().len()
    }
}

/// An axum extractor gating a handler behind a valid session: resolves the request's session
/// cookie to a live, signed-in `AccountSession`, or rejects with a clear client error — before the
/// handler's own logic ever runs. See design.md's "Enforcement" decision for why this lives here,
/// not in `crates/core`.
pub(crate) struct RequireSession {
    pub(crate) account: Arc<AccountSession>,
}

impl RequireSession {
    /// This request's `Scoped` view (process-wide `Shared` + this one account's own
    /// `AccountSession`) — the shape every `commands/*.rs` function expects.
    pub(crate) fn scoped<'a>(&'a self, state: &'a crate::ServerState) -> limusic_core::state::Scoped<'a> {
        limusic_core::state::Scoped { shared: &state.shared, account: &self.account }
    }

    /// The owned counterpart, for the one handler (`lastfm::lastfm_connect`) that spawns a
    /// background task outliving the request.
    pub(crate) fn scoped_owned(&self, state: &crate::ServerState) -> limusic_core::state::ScopedOwned {
        limusic_core::state::ScopedOwned { shared: state.shared.clone(), account: self.account.clone() }
    }
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
        let session_id = session_id_from_headers(&parts.headers).ok_or_else(unauthorized)?;
        let account_key = state.sessions.account_key_for(&session_id).ok_or_else(unauthorized)?;
        // A session minted for an account_key this process no longer has a live AccountSession
        // for (a sign-out, or a restart since) is stale — never auto-created here.
        let account = state.accounts.get(&account_key).ok_or_else(unauthorized)?;
        if !account.it.is_logged_in() {
            return Err(unauthorized());
        }
        Ok(RequireSession { account })
    }
}

/// Resolves a request's session cookie to its live, signed-in `AccountSession` — or `None` if
/// there isn't one, rather than rejecting like `RequireSession` does. For routes that must behave
/// correctly whether or not the caller is signed in: `auth::get_account` (must still answer
/// "signed out" with no session at all), and `media.rs`'s resolve/stream routes, which — under the
/// old single-account-per-process model — implicitly always used the one signed-in identity the
/// whole process happened to hold, signed-in callers and anonymous browser tabs alike. Keeping
/// that same "use my own session when I have one" behavior per caller, now that there can be
/// several different accounts' sessions at once, needs exactly this optional lookup.
pub(crate) fn optional_account(
    state: &crate::ServerState,
    headers: &axum::http::HeaderMap,
) -> Option<Arc<AccountSession>> {
    let session_id = session_id_from_headers(headers)?;
    let account_key = state.sessions.account_key_for(&session_id)?;
    let account = state.accounts.get(&account_key)?;
    account.it.is_logged_in().then_some(account)
}

/// How long a session cookie lasts before the browser forgets it on its own, independent of
/// `AccountRegistry`'s own in-memory lifetime (which ends at the next server restart regardless).
const SESSION_COOKIE_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60; // 30 days

/// Build the `Set-Cookie` header value for a freshly minted session id.
pub(crate) fn session_cookie_header(session_id: &str) -> String {
    // `HttpOnly`: never readable from page JS (design.md's "Auth" decision). `SameSite=Lax`: sent
    // on normal navigation/top-level requests, not on a cross-site POST — reasonable default for a
    // same-site SPA talking to its own API origin. No `Secure` yet: this is a plain-`http://`
    // localhost dev server, same as the rest of this slice (see http-search-vertical-slice's own
    // Non-Goals on hosting/TLS).
    // `Max-Age`: without it, this was a browser "session cookie" — discarded between browsing
    // sessions, which a PWA relaunched from an iOS home-screen icon treats as happening almost
    // every open, forcing a fresh cookie-paste sign-in each time despite nothing being wrong
    // server-side. A real expiry is what "stay signed in" actually requires.
    format!(
        "{SESSION_COOKIE}={session_id}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_COOKIE_MAX_AGE_SECS}"
    )
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
    fn removing_one_session_does_not_affect_another() {
        let store = SessionStore::default();
        let a = store.create("account-a".to_owned());
        let b = store.create("account-b".to_owned());
        store.remove(&a);
        assert_eq!(store.account_key_for(&a), None);
        assert_eq!(
            store.account_key_for(&b),
            Some("account-b".to_owned()),
            "removing one session must not end another's"
        );
    }

    #[test]
    fn any_session_for_reflects_whether_a_session_still_names_that_account() {
        let store = SessionStore::default();
        let a = store.create("account-a".to_owned());
        assert!(store.any_session_for("account-a"));
        store.remove(&a);
        assert!(!store.any_session_for("account-a"));
    }
}
