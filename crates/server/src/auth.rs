// Auth routes (openspec/changes/signed-in-http-parity, tasks 2.2/2.4).
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::session::{clear_session_cookie_header, session_cookie_header, RequireSession};
use crate::{command_err, ApiError, ServerState};

#[derive(Deserialize)]
struct CookieBody {
    cookie: String,
}

/// Cookie-paste sign-in — the only browser login method this server supports (design.md: no
/// webview to host the in-app Google login, cookie-paste is already a supported desktop
/// alternative). Mints an opaque session id distinct from the YouTube cookie itself.
async fn sign_in(
    State(state): State<ServerState>,
    Json(body): Json<CookieBody>,
) -> Result<Response, ApiError> {
    let cookie = body.cookie.trim();
    if cookie.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing or empty `cookie`".to_owned()));
    }
    let account_key = limusic_core::db::account_key(cookie)
        .ok_or((StatusCode::BAD_REQUEST, "that cookie has no SAPISID — not a session cookie".to_owned()))?;
    state.app.sign_in(cookie.to_owned(), None).await.map_err(command_err)?;
    let session_id = state.sessions.create(account_key);
    let snapshot = state.app.account_snapshot();
    let mut response = Json(snapshot).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(&session_id)).unwrap(),
    );
    Ok(response)
}

/// No session required — matches the original Tauri `get_account` command, which has never
/// gated this, and lets an anonymous client tell "signed out" from "sign-in failed" without
/// already holding a session.
async fn get_account(State(state): State<ServerState>) -> Json<serde_json::Value> {
    Json(state.app.account_snapshot())
}

async fn get_account_identities(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    Ok(Json(state.app.account_identities().await.map_err(command_err)?))
}

#[derive(Deserialize)]
struct SwitchBody {
    #[serde(rename = "selectionKey")]
    selection_key: String,
}

async fn switch_account(
    State(state): State<ServerState>,
    _session: RequireSession,
    Json(body): Json<SwitchBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(state.app.switch_account(&body.selection_key).await.map_err(command_err)?))
}

async fn sign_out(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Response {
    state.app.sign_out().await;
    state.sessions.clear();
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&clear_session_cookie_header()).unwrap());
    response
}

/// Saved Google accounts (plural logins, not the one active login's multiple channels — that's
/// `get_account_identities` above). Found missing a route during frontend wiring (task 12.1): the
/// command logic was ported in task 1.4 but never actually exposed over HTTP until now.
async fn get_google_accounts(
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Json<Vec<serde_json::Value>> {
    Json(limusic_core::commands::auth::get_google_accounts(&state.app).await)
}

#[derive(Deserialize)]
struct AccountIdBody {
    id: String,
}

async fn switch_google_account(
    State(state): State<ServerState>,
    _session: RequireSession,
    Json(body): Json<AccountIdBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        limusic_core::commands::auth::switch_google_account(&state.app, &body.id)
            .await
            .map_err(command_err)?,
    ))
}

async fn remove_google_account(
    State(state): State<ServerState>,
    _session: RequireSession,
    Json(body): Json<AccountIdBody>,
) -> StatusCode {
    limusic_core::commands::auth::remove_google_account(&state.app, &body.id).await;
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
