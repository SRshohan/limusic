// Auth routes (openspec/changes/signed-in-http-parity, tasks 2.2/2.4; openspec/changes/
// multi-user-isolation reworked sign-in/sign-out for concurrent accounts).
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use limusic_core::state::Scoped;
use serde::Deserialize;

use crate::session::{clear_session_cookie_header, session_cookie_header, RequireSession};
use crate::{command_err, ApiError, ServerState};

#[derive(Deserialize)]
struct CookieBody {
    cookie: String,
}

/// Cookie-paste sign-in — the only browser login method this server supports (design.md: no
/// webview to host the in-app Google login, cookie-paste is already a supported desktop
/// alternative). Mints an opaque session id distinct from the YouTube cookie itself, and
/// registers (or replaces) that account's live `AccountSession` without touching any other
/// concurrently signed-in account (openspec/changes/multi-user-isolation).
async fn sign_in(
    State(state): State<ServerState>,
    Json(body): Json<CookieBody>,
) -> Result<Response, ApiError> {
    let cookie = body.cookie.trim();
    if cookie.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing or empty `cookie`".to_owned()));
    }
    let (account, _outcome) =
        limusic_core::commands::auth::sign_in(&state.shared, cookie.to_owned())
            .await
            .map_err(command_err)?;
    let account_key = account.account_key.clone();
    let account = state.accounts.insert(account);
    let session_id = state.sessions.create(account_key);
    let snapshot =
        account.account_snapshot(&state.shared);
    let mut response = Json(snapshot).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(&session_id)).unwrap(),
    );
    Ok(response)
}

/// No session required — matches the original Tauri `get_account` command, which has never
/// gated this, and lets an anonymous client tell "signed out" from "sign-in failed" without
/// already holding a session. Unlike every gated route below, this can't use `RequireSession`
/// (it must also answer when there's no session at all), so it resolves the optional session by
/// hand.
async fn get_account(State(state): State<ServerState>, headers: axum::http::HeaderMap) -> Json<serde_json::Value> {
    let account = crate::session::optional_account(&state, &headers);
    let scoped = account.as_ref().map(|account| Scoped { shared: &state.shared, account });
    Json(limusic_core::commands::auth::get_account(scoped.as_ref()).await)
}

async fn get_account_identities(
    State(state): State<ServerState>,
    session: RequireSession,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let scoped = session.scoped(&state);
    Ok(Json(
        limusic_core::commands::auth::get_account_identities(&scoped).await.map_err(command_err)?,
    ))
}

#[derive(Deserialize)]
struct SwitchBody {
    #[serde(rename = "selectionKey")]
    selection_key: String,
}

async fn switch_account(
    State(state): State<ServerState>,
    session: RequireSession,
    Json(body): Json<SwitchBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scoped = session.scoped(&state);
    Ok(Json(
        limusic_core::commands::auth::switch_account(&scoped, &body.selection_key)
            .await
            .map_err(command_err)?,
    ))
}

/// Ends only the calling session — never every other concurrently signed-in account's session
/// (openspec/changes/multi-user-isolation task 4.2). The account's own `AccountSession` is evicted
/// from the shared registry only once no other session still names it.
async fn sign_out(
    State(state): State<ServerState>,
    session: RequireSession,
    request: axum::extract::Request,
) -> Response {
    let scoped = session.scoped(&state);
    limusic_core::commands::auth::sign_out(&scoped).await;
    if let Some(session_id) = crate::session::session_id_from_headers(request.headers()) {
        state.sessions.remove(&session_id);
    }
    if !state.sessions.any_session_for(&session.account.account_key) {
        state.accounts.remove(&session.account.account_key);
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&clear_session_cookie_header()).unwrap());
    response
}

/// Saved Google accounts (plural logins, not the one active login's multiple channels — that's
/// `get_account_identities` above). Narrowed out of openspec/changes/multi-user-isolation (see
/// `AccountSession::google_accounts`'s own doc comment) — always empty on the web build for now.
async fn get_google_accounts(
    State(state): State<ServerState>,
    session: RequireSession,
) -> Json<Vec<serde_json::Value>> {
    let scoped = session.scoped(&state);
    Json(limusic_core::commands::auth::get_google_accounts(&scoped).await)
}

#[derive(Deserialize)]
struct AccountIdBody {
    id: String,
}

async fn switch_google_account(
    State(state): State<ServerState>,
    session: RequireSession,
    Json(body): Json<AccountIdBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scoped = session.scoped(&state);
    Ok(Json(
        limusic_core::commands::auth::switch_google_account(&scoped, &body.id)
            .await
            .map_err(command_err)?,
    ))
}

async fn remove_google_account(
    State(state): State<ServerState>,
    session: RequireSession,
    Json(body): Json<AccountIdBody>,
) -> StatusCode {
    let scoped = session.scoped(&state);
    limusic_core::commands::auth::remove_google_account(&scoped, &body.id).await;
    StatusCode::NO_CONTENT
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/auth/cookie", post(sign_in))
        .route("/api/auth/account", get(get_account))
        .route("/api/auth/identities", get(get_account_identities))
        .route("/api/auth/switch", post(switch_account))
        .route("/api/auth/signout", post(sign_out))
        .route("/api/auth/google-accounts", get(get_google_accounts))
        .route("/api/auth/google-accounts/switch", post(switch_google_account))
        .route("/api/auth/google-accounts/remove", post(remove_google_account))
}
