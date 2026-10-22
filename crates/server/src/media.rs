// Stream resolution + proxy routes (openspec/changes/http-audio-streaming, tasks 4.4-4.5;
// openspec/changes/multi-user-isolation task 5.3/5.4 account-scoped the cache keys and the
// watch-history ping).
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use innertube::AudioQuality;
use limusic_core::media::{audio_proxy, orchestrator::ResolveError, video_proxy};
use limusic_core::state::ANONYMOUS;
use serde::{Deserialize, Serialize};

use crate::session::RequireSession;
use crate::{ApiError, ServerState};

#[derive(Deserialize)]
struct ResolveParams {
    #[serde(rename = "videoId")]
    video_id: String,
    quality: Option<String>,
    #[serde(rename = "isUpload")]
    is_upload: Option<bool>,
}

/// The client-safe subset of `PlaybackData` — never the real upstream URL or its headers
/// (`PlaybackData.headers` is already `#[serde(skip)]`; `stream_url` here is the rewritten proxy
/// URL, not the field of the same name on `PlaybackData`). Watch-history ping registration
/// (`PlaybackData.playback_ping`) is deliberately not wired in this change — out of this task's
/// scope, not silently dropped: a later change registers plays the same way the desktop app does.
#[derive(Serialize)]
struct ResolveResponse {
    video_id: String,
    stream_url: String,
    video_url: Option<String>,
    itag: i64,
    expires_in_seconds: i64,
    loudness_db: Option<f64>,
    title: Option<String>,
    artists: Option<String>,
    duration: Option<String>,
    thumbnail: Option<String>,
    is_video: Option<bool>,
    stream_client: String,
}

fn parse_quality(q: Option<&str>) -> AudioQuality {
    match q.map(str::to_ascii_uppercase).as_deref() {
        Some("LOW") => AudioQuality::Low,
        Some("AUTO") => AudioQuality::Auto,
        _ => AudioQuality::High,
    }
}

fn resolve_err(e: ResolveError) -> ApiError {
    // Distinguish "sign in to play this" from "this track can't be played" (http-media-streaming
    // spec) rather than collapsing every failure to one generic error.
    let status = match e {
        ResolveError::SignInRequired(_) => StatusCode::UNAUTHORIZED,
        ResolveError::UploadUnavailable(_) | ResolveError::LocalMissing(_) => StatusCode::NOT_FOUND,
        ResolveError::Unreachable(_) => StatusCode::BAD_GATEWAY,
        ResolveError::AllClientsFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
    };
    (status, e.to_string())
}

/// This route is anonymous today (no `RequireSession`) — every caller shares the one `ANONYMOUS`
/// cache key (openspec/changes/multi-user-isolation task 5.3). A later change that gates playback
/// behind a session would thread the caller's real `account_key` through here instead.
async fn resolve(
    State(state): State<ServerState>,
    Query(params): Query<ResolveParams>,
) -> Result<Json<ResolveResponse>, ApiError> {
    let video_id = params.video_id.trim();
    if video_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing or empty `videoId` query parameter".to_owned()));
    }
    let quality = parse_quality(params.quality.as_deref());
    let is_upload = params.is_upload.unwrap_or(false);
    let disabled = state.shared.disabled_clients(ANONYMOUS);

    let data = state
        .shared
        .orchestrator
        .resolve(&state.shared.anonymous_it, video_id, is_upload, quality, &disabled)
        .await
        .map_err(resolve_err)?;

    let expires_in = Duration::from_secs(data.expires_in_seconds.max(0) as u64);
    audio_proxy::register(
        ANONYMOUS,
        video_id,
        &data.stream_url,
        data.headers.clone(),
        expires_in,
        data.playback_ping.clone(),
    );

    // The music-video URL (if any) comes from a separate resolve — best-effort, never blocks the
    // audio response on it.
    let video_url = state
        .shared
        .orchestrator
        .resolve_video(&state.shared.anonymous_it, video_id, 1080, &disabled)
        .await
        .inspect(|url| video_proxy::register(ANONYMOUS, video_id, url, expires_in));

    Ok(Json(ResolveResponse {
        video_id: data.video_id,
        stream_url: format!("/media/stream/{video_id}"),
        video_url: video_url.map(|_| format!("/media/video/{video_id}")),
        itag: data.itag,
        expires_in_seconds: data.expires_in_seconds,
        loudness_db: data.loudness_db,
        title: data.title,
        artists: data.artists,
        duration: data.duration,
        thumbnail: data.thumbnail,
        is_video: data.is_video,
        stream_client: data.stream_client,
    }))
}

/// Re-resolve on a cache miss/expiry (a paused-then-resumed track) with safe defaults — a route
/// handler has no queue context to know the original quality/isUpload, so it falls back to the
/// stored setting and `is_upload: false`. A real upload whose cache entry expired re-resolves as
/// an ordinary track and fails cleanly rather than silently misbehaving; the frontend re-calls
/// `/api/playback/resolve` on a proxy error anyway, which re-establishes the correct cache entry.
async fn reresolve_audio(state: &ServerState, video_id: &str) -> Option<()> {
    let disabled = state.shared.disabled_clients(ANONYMOUS);
    let data = state
        .shared
        .orchestrator
        .resolve(&state.shared.anonymous_it, video_id, false, state.shared.quality(ANONYMOUS), &disabled)
        .await
        .ok()?;
    let expires_in = Duration::from_secs(data.expires_in_seconds.max(0) as u64);
    audio_proxy::register(ANONYMOUS, video_id, &data.stream_url, data.headers, expires_in, data.playback_ping);
    Some(())
}

async fn stream_audio(
    State(state): State<ServerState>,
    Path(video_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let mut info = audio_proxy::serve_info(ANONYMOUS, &video_id, range).await;
    if info.is_none() {
        reresolve_audio(&state, &video_id).await;
        info = audio_proxy::serve_info(ANONYMOUS, &video_id, range).await;
    }
    let Some((reg, serve)) = info else {
        return Err(StatusCode::NOT_FOUND);
    };

    let status = if serve.status_partial { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
    let len = serve.end - serve.start + 1;
    let rx = audio_proxy::start_pump(reg, serve.start, serve.end);
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    let mut response = Response::builder()
        .status(status)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, len);
    if let Some(ct) = serve.content_type {
        response = response.header(header::CONTENT_TYPE, ct);
    }
    if serve.status_partial {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", serve.start, serve.end, serve.total),
        );
    }
    response.body(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn stream_video(
    State(state): State<ServerState>,
    Path(video_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let mut url = video_proxy::get(ANONYMOUS, &video_id);
    if url.is_none() {
        let disabled = state.shared.disabled_clients(ANONYMOUS);
        if let Some(u) =
            state.shared.orchestrator.resolve_video(&state.shared.anonymous_it, &video_id, 1080, &disabled).await
        {
            video_proxy::register(ANONYMOUS, &video_id, &u, Duration::from_secs(3600));
            url = Some(u);
        }
    }
    let Some(url) = url else { return Err(StatusCode::NOT_FOUND) };

    let mut out = limusic_core::http::client().get(&url);
    if let Some(range) = headers.get(header::RANGE) {
        out = out.header(header::RANGE.as_str(), range.as_bytes());
    }
    let upstream = out.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    if !upstream.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let mut response = Response::builder()
        .status(upstream.status().as_u16())
        .header(header::ACCEPT_RANGES, "bytes");
    for name in [header::CONTENT_TYPE, header::CONTENT_LENGTH, header::CONTENT_RANGE] {
        if let Some(v) = upstream.headers().get(&name) {
            response = response.header(name, v);
        }
    }
    let stream = upstream.bytes_stream();
    response
        .body(Body::from_stream(stream))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize)]
struct RegisterParams {
    #[serde(rename = "videoId")]
    video_id: String,
}

/// Watch-history ping — the piece `resolve`'s own doc comment on `ResolveResponse` called out as
/// deliberately deferred ("a later change registers plays the same way the desktop app does").
/// The frontend calls this once playback crosses the same threshold desktop's
/// `AppState::on_position` uses (half the track's duration, capped at 30s — `historyThreshold` in
/// web-playback.svelte.ts). Requires a session: watch history only means anything against a real
/// signed-in account — and under multi-user isolation, the caller's own `account_key` is what
/// selects the right cache entry and the right `InnerTube` to send the ping through. Best-effort
/// and idempotent either way: `audio_proxy::take_ping` latches so a retried/duplicate call can't
/// double-register, and a missing/expired registration or a client YouTube never gave a tracking
/// URL to both just no-op rather than error — same as desktop logging-and-ignoring this exact
/// failure mode.
async fn register_play(
    State(state): State<ServerState>,
    session: RequireSession,
    Json(body): Json<RegisterParams>,
) -> StatusCode {
    let account_key = &session.account.account_key;
    let Some((ping, cpn)) = audio_proxy::take_ping(account_key, &body.video_id) else {
        return StatusCode::NO_CONTENT;
    };
    let Some(client) = state.shared.clients.get(&ping.client) else {
        tracing::warn!(client = %ping.client, "watch-history ping: unknown client");
        return StatusCode::NO_CONTENT;
    };
    match session.account.it.register_playback(client, &ping.url, &cpn, None).await {
        Ok(()) => tracing::debug!(client = %ping.client, "watch-history ping sent"),
        Err(e) => tracing::warn!(error = %e, "watch-history ping failed"),
    }
    StatusCode::NO_CONTENT
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/playback/resolve", get(resolve))
        .route("/api/playback/register", post(register_play))
        .route("/media/stream/{videoId}", get(stream_audio))
        .route("/media/video/{videoId}", get(stream_video))
}
