//! Integration tests driving the real axum router in-process (openspec/changes/
//! multi-user-isolation tasks 4.1-4.3, 5.4, 7.1). Two synthetic accounts stand in for two real
//! signed-in users: `AccountSession::anonymous` + a locally-set fake cookie is enough to satisfy
//! `RequireSession` (`InnerTube::is_logged_in` only checks whether a cookie string was set, never
//! the network — see `innertube::transport`), so these never touch YouTube. A genuine sign-in
//! round-trip needs a live account cookie, which this environment doesn't have (see tasks.md's
//! 7.3 note) — that gap is a test-environment limitation, not something these tests paper over.
#![cfg(test)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use innertube::Clients;
use limusic_core::media::audio_proxy;
use limusic_core::media::orchestrator::PlaybackPing;
use limusic_core::state::{AccountSession, Shared};
use tower::ServiceExt;

use crate::session::{AccountRegistry, SessionStore, SESSION_COOKIE};
use crate::{app, ServerState};

/// A locally "signed-in" account that makes no network calls — see this module's doc comment.
fn fake_account(account_key: &str) -> AccountSession {
    let account = AccountSession::anonymous(account_key.to_owned());
    account.it.set_cookie(Some(format!("SAPISID={account_key}; SID=fake")));
    account
}

/// A fresh, isolated `ServerState` (in-memory DB, scratch cipher-cache dir) with two live,
/// concurrently signed-in accounts already registered and their session ids minted.
struct Fixture {
    state: ServerState,
    session_a: String,
    session_b: String,
}

impl Fixture {
    async fn new() -> Self {
        let db = Arc::new(limusic_core::db::Db::open(std::path::Path::new(":memory:")).unwrap());
        let lastfm = limusic_core::lastfm::spawn(None);
        let app_data_dir = std::env::temp_dir()
            .join(format!("limusic-server-test-{}-{}", std::process::id(), fastrand_suffix()));
        std::fs::create_dir_all(&app_data_dir).unwrap();
        let shared = Arc::new(Shared::new(Clients::bundled(), db, lastfm, &app_data_dir));

        let accounts = Arc::new(AccountRegistry::default());
        let sessions = Arc::new(SessionStore::default());
        let account_a = accounts.insert(fake_account("acct-a"));
        let account_b = accounts.insert(fake_account("acct-b"));
        let session_a = sessions.create(account_a.account_key.clone());
        let session_b = sessions.create(account_b.account_key.clone());

        let state = ServerState { shared, accounts, sessions };
        Fixture { state, session_a, session_b }
    }

    fn router(&self) -> axum::Router {
        app(self.state.clone())
    }
}

fn fastrand_suffix() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h);
    std::thread::current().id().hash(&mut h);
    h.finish()
}

fn with_session(builder: axum::http::request::Builder, session_id: &str) -> axum::http::request::Builder {
    builder.header("cookie", format!("{SESSION_COOKIE}={session_id}"))
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Core scenario (task 7.1 / design.md): two accounts signed in concurrently, each changes a
/// setting, and each one's own listing reflects only its own.
#[tokio::test]
async fn settings_are_isolated_between_concurrently_signed_in_accounts() {
    let fx = Fixture::new().await;

    for (session_id, value) in [(&fx.session_a, "LOW"), (&fx.session_b, "HIGH")] {
        let request = with_session(Request::builder().method("PUT").uri("/api/settings/quality"), session_id)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({ "value": value }).to_string()))
            .unwrap();
        let response = fx.router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    for (session_id, expected) in [(&fx.session_a, "LOW"), (&fx.session_b, "HIGH")] {
        let request =
            with_session(Request::builder().method("GET").uri("/api/settings/quality"), session_id)
                .body(Body::empty())
                .unwrap();
        let response = fx.router().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_json(response).await, serde_json::json!(expected));
    }
}

/// task 4.2: signing out one account's session must not disturb another concurrently signed-in
/// account's session.
#[tokio::test]
async fn signing_out_one_session_does_not_affect_another() {
    let fx = Fixture::new().await;

    let sign_out = with_session(Request::builder().method("POST").uri("/api/auth/signout"), &fx.session_a)
        .body(Body::empty())
        .unwrap();
    let response = fx.router().oneshot(sign_out).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // A's session is gone...
    let still_a = with_session(Request::builder().method("GET").uri("/api/settings/quality"), &fx.session_a)
        .body(Body::empty())
        .unwrap();
    let response = fx.router().oneshot(still_a).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // ...but B's keeps working.
    let still_b = with_session(Request::builder().method("GET").uri("/api/settings/quality"), &fx.session_b)
        .body(Body::empty())
        .unwrap();
    let response = fx.router().oneshot(still_b).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// task 5.4: `POST /api/playback/register` must claim only the calling account's own watch-history
/// ping, never another concurrently signed-in account's registration for the same `videoId`. The
/// fake client name is deliberately unknown to `Clients` so the handler no-ops before attempting
/// any network call, while still exercising the real route → `RequireSession` → `take_ping` path.
#[tokio::test]
async fn playback_register_only_claims_the_calling_accounts_own_ping() {
    let fx = Fixture::new().await;
    let video_id = "shared-video-id";
    let ping = PlaybackPing { url: "http://127.0.0.1:0/unused".into(), client: "FAKE_TEST_CLIENT".into() };
    audio_proxy::register(
        "acct-a",
        video_id,
        "https://example.invalid/a",
        Default::default(),
        std::time::Duration::from_secs(60),
        Some(ping.clone()),
    );
    audio_proxy::register(
        "acct-b",
        video_id,
        "https://example.invalid/b",
        Default::default(),
        std::time::Duration::from_secs(60),
        Some(ping),
    );

    let request = with_session(Request::builder().method("POST").uri("/api/playback/register"), &fx.session_a)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({ "videoId": video_id }).to_string()))
        .unwrap();
    let response = fx.router().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    assert!(
        audio_proxy::take_ping("acct-a", video_id).is_none(),
        "account A's own ping must have been claimed by its own request"
    );
    assert!(
        audio_proxy::take_ping("acct-b", video_id).is_some(),
        "account B's ping for the same videoId must be untouched by A's request"
    );
}
