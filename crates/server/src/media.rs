// Stream resolution + proxy routes (openspec/changes/http-audio-streaming, tasks 4.4-4.5).
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use innertube::AudioQuality;
use limusic_core::media::{audio_proxy, orchestrator::ResolveError, video_proxy};
use serde::{Deserialize, Serialize};

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
    let disabled = state.app.disabled_clients();

    let data = state
        .app
        .orchestrator
        .resolve(video_id, is_upload, quality, &disabled)
        .await
        .map_err(resolve_err)?;

    let expires_in = Duration::from_secs(data.expires_in_seconds.max(0) as u64);
    audio_proxy::register(video_id, &data.stream_url, data.headers.clone(), expires_in);

    // The music-video URL (if any) comes from a separate resolve — best-effort, never blocks the
    // audio response on it.
    let video_url = state
        .app
        .orchestrator
        .resolve_video(video_id, 1080, &disabled)
        .await
        .inspect(|url| video_proxy::register(video_id, url, expires_in));

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
    let disabled = state.app.disabled_clients();
    let data = state
        .app
        .orchestrator
        .resolve(video_id, false, state.app.quality(), &disabled)
        .await
        .ok()?;
    let expires_in = Duration::from_secs(data.expires_in_seconds.max(0) as u64);
    audio_proxy::register(video_id, &data.stream_url, data.headers, expires_in);
    Some(())
}

async fn stream_audio(
    State(state): State<ServerState>,
    Path(video_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let mut info = audio_proxy::serve_info(&video_id, range).await;
    if info.is_none() {
        reresolve_audio(&state, &video_id).await;
        info = audio_proxy::serve_info(&video_id, range).await;
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
    let mut url = video_proxy::get(&video_id);
    if url.is_none() {
        let disabled = state.app.disabled_clients();
        if let Some(u) = state.app.orchestrator.resolve_video(&video_id, 1080, &disabled).await {
            video_proxy::register(&video_id, &u, Duration::from_secs(3600));
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

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/playback/resolve", get(resolve))
        .route("/media/stream/{videoId}", get(stream_audio))
        .route("/media/video/{videoId}", get(stream_video))
}
