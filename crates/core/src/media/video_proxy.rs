//! Range proxy for resolved YouTube music-video streams (openspec/changes/http-audio-streaming,
//! design.md D4) — ported from `src-tauri/src/videoproxy.rs`. Much simpler than
//! [`crate::media::audio_proxy`]: the `<video>` element's own `Range` header passes straight
//! through to googlevideo (no bounded-chunk throttle workaround needed for this path — the
//! desktop version never had one here either), so this is a thin pass-through, not a chunked pump.
//!
//! Same cache-key simplification as `audio_proxy`: videoId instead of a random id + startup token
//! (that indirection guarded against an arbitrary local process on the loopback interface, not a
//! relevant threat behind a real HTTP origin).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Entry {
    url: String,
    expires_at: Instant,
}

static CACHE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Entry>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register the resolved video URL for `video_id`, replacing any existing entry.
pub fn register(video_id: &str, url: &str, expires_in: Duration) {
    cache().lock().unwrap().insert(
        video_id.to_owned(),
        Entry { url: url.to_owned(), expires_at: Instant::now() + expires_in },
    );
}

/// The live video URL for `video_id`, or `None` if never registered or past its expiry.
pub fn get(video_id: &str) -> Option<String> {
    let guard = cache().lock().unwrap();
    let entry = guard.get(video_id)?;
    (Instant::now() < entry.expires_at).then(|| entry.url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_then_get_round_trips_within_its_expiry() {
        register("video-abc", "https://x.googlevideo.com/videoplayback", Duration::from_secs(60));
        assert_eq!(get("video-abc").as_deref(), Some("https://x.googlevideo.com/videoplayback"));
    }

    #[test]
    fn an_expired_registration_reads_as_a_miss() {
        register("video-expired", "https://x.googlevideo.com/videoplayback", Duration::from_secs(0));
        std::thread::sleep(Duration::from_millis(5));
        assert!(get("video-expired").is_none());
    }
}
