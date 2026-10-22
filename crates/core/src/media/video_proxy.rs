//! Range proxy for resolved YouTube music-video streams (openspec/changes/http-audio-streaming,
//! design.md D4) — ported from `src-tauri/src/videoproxy.rs`. Much simpler than
//! [`crate::media::audio_proxy`]: the `<video>` element's own `Range` header passes straight
//! through to googlevideo (no bounded-chunk throttle workaround needed for this path — the
//! desktop version never had one here either), so this is a thin pass-through, not a chunked pump.
//!
//! Same cache-key simplification as `audio_proxy`: videoId instead of a random id + startup token
//! (that indirection guarded against an arbitrary local process on the loopback interface, not a
//! relevant threat behind a real HTTP origin).
//!
//! Keyed by `(account_key, video_id)`, not `video_id` alone (openspec/changes/multi-user-isolation
//! Decision 3) — same reasoning as `audio_proxy`: two accounts resolving the same video around the
//! same time must not overwrite each other's entry.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Entry {
    url: String,
    expires_at: Instant,
}

type CacheKey = (String, String);

static CACHE: OnceLock<Mutex<HashMap<CacheKey, Entry>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<CacheKey, Entry>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register the resolved video URL for `(account_key, video_id)`, replacing any existing entry for
/// that exact pair.
pub fn register(account_key: &str, video_id: &str, url: &str, expires_in: Duration) {
    cache().lock().unwrap().insert(
        (account_key.to_owned(), video_id.to_owned()),
        Entry { url: url.to_owned(), expires_at: Instant::now() + expires_in },
    );
}

/// The live video URL for `(account_key, video_id)`, or `None` if never registered or past its
/// expiry.
pub fn get(account_key: &str, video_id: &str) -> Option<String> {
    let guard = cache().lock().unwrap();
    let entry = guard.get(&(account_key.to_owned(), video_id.to_owned()))?;
    (Instant::now() < entry.expires_at).then(|| entry.url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_then_get_round_trips_within_its_expiry() {
        register("acct", "video-abc", "https://x.googlevideo.com/videoplayback", Duration::from_secs(60));
        assert_eq!(get("acct", "video-abc").as_deref(), Some("https://x.googlevideo.com/videoplayback"));
    }

    #[test]
    fn an_expired_registration_reads_as_a_miss() {
        register("acct", "video-expired", "https://x.googlevideo.com/videoplayback", Duration::from_secs(0));
        std::thread::sleep(Duration::from_millis(5));
        assert!(get("acct", "video-expired").is_none());
    }

    #[test]
    fn two_accounts_registering_the_same_video_id_do_not_clobber_each_other() {
        register("acct-a", "shared-video", "https://x/a", Duration::from_secs(60));
        register("acct-b", "shared-video", "https://x/b", Duration::from_secs(60));
        assert_eq!(get("acct-a", "shared-video").as_deref(), Some("https://x/a"));
        assert_eq!(get("acct-b", "shared-video").as_deref(), Some("https://x/b"));
    }
}
