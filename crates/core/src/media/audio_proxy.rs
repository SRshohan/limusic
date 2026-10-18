//! Throttling-safe range proxy for resolved YouTube audio streams (openspec/changes/
//! http-audio-streaming, design.md D4) — ported from `src-tauri/src/audioproxy.rs`'s bounded-range
//! reissue logic (the actual fix for googlevideo's throttling of open-ended ranges: measured there
//! at 32 KB/s open-ended vs. 9.4 MB/s for a 4 MiB bounded range).
//!
//! What's different from the desktop version, and why: that file ran its own loopback hyper server
//! (mpv had no other way to reach it) behind a random-id-per-registration + process-startup-token
//! scheme, whose whole purpose was stopping an arbitrary *local* process from guessing a loopback
//! port. Behind a real HTTP server with its own origin and CORS policy, that threat doesn't apply,
//! so this is a plain in-memory cache keyed by **videoId** instead — `crates/server`'s axum routes
//! own the actual serving (no hyper here at all). Entries carry an expiry
//! (`PlaybackData.expires_in_seconds`), since a videoId can be looked up long after it was
//! resolved (a paused track resumed later) — a stale entry is treated as a cache miss, and the
//! route handler re-resolves rather than proxy a dead upstream URL.
//!
//! The actual range/pump logic (`pump_chunks`, `chunk_end`, `parse_range`/`resolve_window`/`Want`,
//! `upstream_request`/`upstream_get`) is ported unchanged — pure `reqwest` + channel plumbing, no
//! hyper server dependency to begin with.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::header;
use tokio::sync::mpsc;

use super::orchestrator::PlaybackPing;

/// How much of the file to ask upstream for in one range request. Large enough to amortise the
/// per-request round trip and land in the "served at full speed" regime, small enough that a seek
/// abandons little work when the client drops the connection.
const CHUNK: u64 = 4 * 1024 * 1024;

/// How long one upstream read may make no progress before the chunk is declared dead. A stall
/// guard, not a total-request timeout: a slow-but-moving download is never cut off, but a
/// connection that goes quiet without closing doesn't hang the pump forever either.
const STALL: Duration = Duration::from_secs(20);

/// Keep at most this many stream URLs registered. A registration is the URL plus its headers (a
/// cookie on an upload), so this is a few KB; the cache only has to be deeper than the current and
/// gapless-next tracks, which are always the two most recent.
const MAX_REGISTRATIONS: usize = 64;

/// `send()` (or one body read) with the stall guard on it.
pub(crate) async fn no_stall<F: std::future::Future<Output = T>, T>(f: F) -> Result<T, io::Error> {
    tokio::time::timeout(STALL, f).await.map_err(|_| io::Error::other("upstream stalled"))
}

/// What a resolved URL answers: its total length and content type. Learned from the first chunk
/// response's `Content-Range` and reused for every later connection, so a seek does not pay a
/// separate round trip just to learn the size.
#[derive(Clone)]
struct Meta {
    total: u64,
    content_type: Option<String>,
}

/// Crosses the `crates/server` crate boundary (held by [`ServeInfo`]/[`serve_info`]'s callers), so
/// `pub`, not `pub(crate)` — this is a separate crate in the same workspace, not the same crate.
pub struct Registration {
    url: String,
    headers: HashMap<String, String>,
    expires_at: Instant,
    meta: Mutex<Option<Meta>>,
    /// The watch-history ping for this exact play (openspec/changes/http-audio-streaming's own
    /// deferred-scope note: "a later change registers plays the same way the desktop app does" —
    /// this is that change). `None` when no client's `/player` response carried a tracking URL,
    /// same as desktop's `state.rs::on_position`.
    ping: Option<PlaybackPing>,
    /// One per registration (= one per play, including a re-resolve after expiry — correctly a
    /// *new* play): `innertube::generate_cpn()`, mirrors desktop's `q.cpn` reset on every new play.
    cpn: String,
    /// Latches the ping to fire at most once per registration, mirroring desktop's
    /// `AppState::history_pinged` atomic.
    pinged: AtomicBool,
}

struct Registry {
    map: HashMap<String, Arc<Registration>>,
    order: VecDeque<String>,
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry { map: HashMap::new(), order: VecDeque::new() }))
}

/// Register a resolved stream for `video_id`, replacing any existing entry for it. `expires_in`
/// comes straight from `PlaybackData.expires_in_seconds`. `ping` is `PlaybackData.playback_ping` —
/// pass it through even when `None` (a re-registration must drop a stale ping, not keep one from
/// a different client's response).
pub fn register(
    video_id: &str,
    url: &str,
    headers: HashMap<String, String>,
    expires_in: Duration,
    ping: Option<PlaybackPing>,
) {
    let reg = Arc::new(Registration {
        url: url.to_owned(),
        headers,
        expires_at: Instant::now() + expires_in,
        meta: Mutex::new(None),
        ping,
        cpn: innertube::generate_cpn(),
        pinged: AtomicBool::new(false),
    });
    let mut r = registry().lock().unwrap();
    if !r.map.contains_key(video_id) {
        while r.order.len() >= MAX_REGISTRATIONS {
            if let Some(old) = r.order.pop_front() {
                r.map.remove(&old);
            }
        }
        r.order.push_back(video_id.to_owned());
    }
    r.map.insert(video_id.to_owned(), reg);
}

/// Claim the one-time right to send `video_id`'s watch-history ping — `None` if there's no ping to
/// send (no client's response carried a tracking URL, no live registration), or if something
/// already claimed it for this play. The caller (`crates/server`) does the actual
/// `register_playback` HTTP call; this module only owns the latch, the same split desktop's
/// `AppState::on_position` draws between deciding and sending.
pub fn take_ping(video_id: &str) -> Option<(PlaybackPing, String)> {
    let reg = get(video_id)?;
    let ping = reg.ping.clone()?;
    (!reg.pinged.swap(true, Ordering::Relaxed)).then_some((ping, reg.cpn.clone()))
}

/// The live registration for `video_id`, or `None` if never registered or past its expiry (the
/// route handler re-resolves on a miss — see design.md D4).
pub fn get(video_id: &str) -> Option<Arc<Registration>> {
    let reg = registry().lock().unwrap().map.get(video_id).cloned()?;
    (Instant::now() < reg.expires_at).then_some(reg)
}

/// The cached total length + content type, probing upstream once per registration if nothing has
/// learned it yet.
async fn meta_for(reg: &Registration) -> Result<Meta, &'static str> {
    if let Some(m) = reg.meta.lock().ok().and_then(|g| g.clone()) {
        return Ok(m);
    }
    let m = probe(reg).await?;
    if let Ok(mut g) = reg.meta.lock() {
        *g = Some(m.clone());
    }
    Ok(m)
}

/// Ask upstream for a single byte (`bytes=0-0`) just to read the total length and content type off
/// the answer.
async fn probe(reg: &Registration) -> Result<Meta, &'static str> {
    let resp = upstream_get(reg, 0, 0).await?;
    if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let total = resp
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(total_from_content_range)
            .ok_or("bad content-range")?;
        Ok(Meta { total, content_type: content_type(&resp) })
    } else {
        Err("upstream did not honor a bounded range")
    }
}

fn upstream_request(reg: &Registration, start: u64, end: u64) -> reqwest::RequestBuilder {
    let mut req = crate::http::client()
        .get(&reg.url)
        .header(header::RANGE.as_str(), format!("bytes={start}-{end}"))
        .header(header::ACCEPT_ENCODING.as_str(), "identity");
    for (k, v) in &reg.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    req
}

async fn upstream_get(reg: &Registration, start: u64, end: u64) -> Result<reqwest::Response, &'static str> {
    let sent = no_stall(upstream_request(reg, start, end).send()).await.map_err(|_| "upstream stalled")?;
    sent.map_err(|_| "upstream failed")
}

fn content_type(resp: &reqwest::Response) -> Option<String> {
    resp.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

fn total_from_content_range(value: &str) -> Option<u64> {
    // `bytes 0-0/122414474`, or `bytes 0-0/*` when the length is unknown.
    value.rsplit_once('/')?.1.trim().parse().ok()
}

/// The last byte to ask for in a chunk that starts at `pos`, clamped to the window's last byte.
fn chunk_end(pos: u64, end: u64) -> u64 {
    (pos + CHUNK - 1).min(end)
}

/// Start pumping `[start, end]` (inclusive) from `reg` in bounded upstream chunks, returning the
/// receiving half of the channel — `crates/server`'s axum route turns this into a streamed
/// response body. Dropping the receiver (the client disconnected, or a seek replaced it) cancels
/// the in-flight upstream request.
pub fn start_pump(
    reg: Arc<Registration>,
    start: u64,
    end: u64,
) -> mpsc::Receiver<Result<Bytes, io::Error>> {
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(pump_chunks(reg, start, end, tx));
    rx
}

/// Walk `pos` to `end` inclusive in bounded upstream ranges, forwarding each response's bytes into
/// the channel. Ends when the window is done or the receiver is gone (a seek dropped the
/// connection), which is also what cancels the in-flight upstream request.
async fn pump_chunks(
    reg: Arc<Registration>,
    start: u64,
    end: u64,
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
) {
    let mut pos = start;
    while pos <= end {
        let last = chunk_end(pos, end);
        let resp = match no_stall(upstream_request(&reg, pos, last).send()).await {
            Ok(Ok(r)) if r.status() == reqwest::StatusCode::PARTIAL_CONTENT => r,
            Ok(Ok(r)) => {
                let _ = tx
                    .send(Err(io::Error::other(format!("audio proxy: upstream status {}", r.status()))))
                    .await;
                return;
            }
            Ok(Err(e)) => {
                let _ = tx.send(Err(io::Error::other(e))).await;
                return;
            }
            Err(e) => {
                let _ = tx.send(Err(e)).await;
                return;
            }
        };
        pos = last + 1;
        let mut body = resp.bytes_stream();
        loop {
            let item = match no_stall(body.next()).await {
                Ok(Some(item)) => item.map_err(io::Error::other),
                Ok(None) => break,
                Err(e) => Err(e),
            };
            let failed = item.is_err();
            if tx.send(item).await.is_err() {
                return; // client gone, stop pulling and drop the connection upstream
            }
            if failed {
                return;
            }
        }
    }
}

/// What the client asked for, once parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Want {
    /// No `Range` header, or one this proxy does not understand: the whole file.
    Full,
    /// `bytes=X-`
    From(u64),
    /// `bytes=X-Y`
    Bounded(u64, u64),
    /// `bytes=-N` (the last N bytes)
    Suffix(u64),
}

pub(crate) fn parse_range(value: Option<&str>) -> Want {
    let Some(v) = value else { return Want::Full };
    let Some(spec) = v.strip_prefix("bytes=") else { return Want::Full };
    let spec = spec.split(',').next().unwrap_or(spec).trim();
    let Some((a, b)) = spec.split_once('-') else { return Want::Full };
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        return match b.parse() {
            Ok(n) => Want::Suffix(n),
            Err(_) => Want::Full,
        };
    }
    let Ok(start) = a.parse() else { return Want::Full };
    if b.is_empty() {
        Want::From(start)
    } else {
        match b.parse::<u64>() {
            Ok(end) if end >= start => Want::Bounded(start, end),
            _ => Want::Full,
        }
    }
}

/// The inclusive window to serve for `want`, or `None` when the request cannot be satisfied.
pub(crate) fn resolve_window(want: Want, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    match want {
        Want::Full => Some((0, total - 1)),
        Want::From(s) => (s < total).then_some((s, total - 1)),
        Want::Bounded(s, e) => (s < total).then(|| (s, e.min(total - 1))),
        Want::Suffix(n) => (n > 0).then(|| (total.saturating_sub(n), total - 1)),
    }
}

/// What an axum route needs to build its response: status shape, headers, and the byte range to
/// pump. Separated from the actual `axum::response::Response` construction so this module stays
/// framework-agnostic (`crates/server` owns the axum-specific plumbing).
pub struct ServeInfo {
    pub status_partial: bool,
    pub start: u64,
    pub end: u64,
    pub total: u64,
    pub content_type: Option<String>,
}

/// Resolve what to serve for `video_id` given a `Range` header value, without starting the actual
/// byte pump. `None` on a cache miss (caller re-resolves) or an unsatisfiable range.
pub async fn serve_info(video_id: &str, range: Option<&str>) -> Option<(Arc<Registration>, ServeInfo)> {
    let reg = get(video_id)?;
    let meta = meta_for(&reg).await.ok()?;
    let want = parse_range(range);
    let (start, end) = resolve_window(want, meta.total)?;
    Some((
        reg,
        ServeInfo {
            status_partial: !matches!(want, Want::Full),
            start,
            end,
            total: meta.total,
            content_type: meta.content_type,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_parse_the_ways_a_browser_audio_element_sends_them() {
        assert_eq!(parse_range(None), Want::Full);
        assert_eq!(parse_range(Some("bytes=0-")), Want::From(0));
        assert_eq!(parse_range(Some("bytes=1234-")), Want::From(1234));
        assert_eq!(parse_range(Some("bytes=10-19")), Want::Bounded(10, 19));
        assert_eq!(parse_range(Some("bytes=0-0")), Want::Bounded(0, 0));
        assert_eq!(parse_range(Some("bytes=-500")), Want::Suffix(500));
        assert_eq!(parse_range(Some("items=0-")), Want::Full);
        assert_eq!(parse_range(Some("bytes=oops")), Want::Full);
        assert_eq!(parse_range(Some("bytes=19-10")), Want::Full);
    }

    #[test]
    fn windows_clamp_and_reject() {
        assert_eq!(resolve_window(Want::Full, 1000), Some((0, 999)));
        assert_eq!(resolve_window(Want::From(0), 1000), Some((0, 999)));
        assert_eq!(resolve_window(Want::From(500), 1000), Some((500, 999)));
        assert_eq!(resolve_window(Want::Bounded(0, 5000), 1000), Some((0, 999)));
        assert_eq!(resolve_window(Want::Suffix(100), 1000), Some((900, 999)));
        assert_eq!(resolve_window(Want::From(1000), 1000), None);
        assert_eq!(resolve_window(Want::Suffix(0), 1000), None);
        assert_eq!(resolve_window(Want::Full, 0), None);
    }

    /// The chunks have to tile the window exactly: no gap to stall on, no overlap to duplicate.
    #[test]
    fn chunks_tile_the_window() {
        assert_eq!(chunk_end(0, 10 * 1024 * 1024), CHUNK - 1);
        assert_eq!(chunk_end(CHUNK, 10 * 1024 * 1024), 2 * CHUNK - 1);
        assert_eq!(chunk_end(8 * 1024 * 1024, 10 * 1024 * 1024), 10 * 1024 * 1024);
        assert_eq!(chunk_end(0, 99), 99);

        let mut pos = 0u64;
        let end = 9 * 1024 * 1024 + 123;
        let mut seen = 0u64;
        while pos <= end {
            let last = chunk_end(pos, end);
            assert!(last <= end);
            seen += last - pos + 1;
            pos = last + 1;
        }
        assert_eq!(pos, end + 1);
        assert_eq!(seen, end + 1);
    }

    #[test]
    fn total_is_read_from_content_range() {
        assert_eq!(total_from_content_range("bytes 0-0/122414474"), Some(122414474));
        assert_eq!(total_from_content_range("bytes 0-0/*"), None);
        assert_eq!(total_from_content_range("nonsense"), None);
    }

    #[test]
    fn register_then_get_round_trips_within_its_expiry() {
        register("abc123", "https://x.googlevideo.com/videoplayback", HashMap::new(), Duration::from_secs(60), None);
        let reg = get("abc123").expect("just registered");
        assert_eq!(reg.url, "https://x.googlevideo.com/videoplayback");
    }

    #[test]
    fn an_expired_registration_reads_as_a_miss() {
        register("expired-entry", "https://x.googlevideo.com/videoplayback", HashMap::new(), Duration::from_secs(0), None);
        std::thread::sleep(Duration::from_millis(5));
        assert!(get("expired-entry").is_none(), "past its expiry, a registration must not be served");
    }

    #[test]
    fn re_registering_the_same_video_id_does_not_grow_the_fifo() {
        for _ in 0..3 {
            register("same-id", "https://x.googlevideo.com/videoplayback", HashMap::new(), Duration::from_secs(60), None);
        }
        let r = registry().lock().unwrap();
        assert_eq!(r.order.iter().filter(|id| *id == "same-id").count(), 1);
    }

    #[test]
    fn take_ping_fires_at_most_once_per_registration() {
        let ping = PlaybackPing { url: "https://x/ptracking".to_owned(), client: "WEB_REMIX".to_owned() };
        register("ping-id", "https://x.googlevideo.com/videoplayback", HashMap::new(), Duration::from_secs(60), Some(ping.clone()));
        let (first, _cpn) = take_ping("ping-id").expect("a ping was registered");
        assert_eq!(first.url, ping.url);
        assert!(take_ping("ping-id").is_none(), "a second claim on the same play must find nothing left");
    }

    #[test]
    fn take_ping_is_none_when_no_client_returned_a_tracking_url() {
        register("no-ping-id", "https://x.googlevideo.com/videoplayback", HashMap::new(), Duration::from_secs(60), None);
        assert!(take_ping("no-ping-id").is_none());
    }
}
