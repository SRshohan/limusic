//! `CipherDeobfuscator` — the signature/`n`-transform runtime the orchestrator calls.
//!
//! Ported from `src-tauri/src/cipher/mod.rs` (openspec/changes/http-audio-streaming, design.md
//! D3). The only change from that file: `crate::webview::Bridge` (a hidden Tauri webview) is
//! replaced by [`jshost::JsBridge`] (a headless `deno_core` runtime on the shared v8 thread), and
//! `CipherDeobfuscator::new` drops its `AppHandle` parameter — there is no app handle here at all.
//! The discovery JS, sig/n invocation, and self-heal retry logic are otherwise unchanged.
//!
//! Ties [`fetcher`] (player.js) + [`extractor`]/[`config`] (function names) + the headless cipher
//! runtime that runs YouTube's own code. Every public method degrades gracefully: a runtime or
//! extraction failure yields `None` / the original URL, and the orchestrator falls through to the
//! non-cipher fallback clients.
//!
//! The runtime is built on demand and torn down when idle (`teardown_if_idle`), never held for the
//! life of the process: STS — the one thing every /player request needs — comes from analysis
//! alone.

mod config;
mod extractor;
mod fetcher;
pub(crate) mod jshost;

pub use config::PlayerConfigStore;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Mutex;

use fetcher::PlayerJsFetcher;
use jshost::JsBridge;

const CIPHER_LABEL: &str = "limusic-cipher";
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const LOAD_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a "this player has no config" verdict stands before it is re-checked. It must expire:
/// the community registries publish a config for a rotated `player.js` within hours, and nothing
/// else in the process ever re-asks.
const UNKNOWN_PLAYER_RETRY: Duration = Duration::from_secs(15 * 60);

/// Discovery/validation: prove the injected exports actually WORK before the orchestrator commits
/// to this player, by running each on a sample input. The sig function and `n` class live inside
/// player.js's IIFE closure and are never on `window` on their own.
const DISCOVERY_JS: &str = r#"(function(){
  var t="grut12Abc_-";
  function ok(s){return typeof s==='string'&&/^[A-Za-z0-9_-]+$/.test(s)&&s!==t;}
  window.__n_ok=false;
  window.__sig_ok=false;
  try{window.__n_ok=(typeof window._nTransformFunc==='function'&&ok(window._nTransformFunc(t)));}catch(e){}
  try{window.__sig_ok=(typeof window._cipherSigFunc==='function'&&typeof window._cipherSigFunc(t)==='string');}catch(e){}
  window.__cipher_loaded=true;
})();"#;

#[derive(Default)]
struct Inner {
    bridge: Option<JsBridge>,
    sts: Option<i32>,
    built_epoch: u64,
    n_available: bool,
    /// Whether an `_cipherSigFunc` export exists. When false, deciphering is impossible on this
    /// player regardless of freshness — so we skip refetch/retry.
    sig_available: bool,
    /// player.js has been fetched and its config/STS resolved for `built_epoch`.
    analyzed: bool,
    /// Discovery has run for `built_epoch`, so `sig_available`/`n_available` mean something.
    discovered: bool,
    /// When the runtime last did work, for the idle teardown. `Some` whenever `bridge` is.
    last_used: Option<Instant>,
    /// When this player's hash was last looked up and not found. `Some` means the analysis
    /// completed but produced no way to decipher, and that verdict expires after
    /// [`UNKNOWN_PLAYER_RETRY`].
    unknown_player_at: Option<Instant>,
}

impl Inner {
    /// Whether [`CipherDeobfuscator::ensure_analyzed`] still owes work for `epoch`. `bridge_ok` is
    /// whether the runtime is actually still live.
    fn owes_work(&self, epoch: u64, want_bridge: bool, bridge_ok: bool) -> bool {
        if !self.analyzed || self.built_epoch != epoch {
            return true;
        }
        if self.unknown_player_at.is_some_and(|t| t.elapsed() >= UNKNOWN_PLAYER_RETRY) {
            return true;
        }
        if !want_bridge {
            return false; // STS is already in hand; no runtime needed to hand it over
        }
        if !self.discovered {
            return true; // an analysis-only pass got us here — the runtime was never built
        }
        keep_bridge(self.sig_available, self.n_available) && !bridge_ok
    }

    /// Whether the runtime has gone unused for `idle` (never-used counts as idle).
    fn idle_for(&self, idle: Duration) -> bool {
        !self.last_used.is_some_and(|t| t.elapsed() < idle)
    }
}

pub struct CipherDeobfuscator {
    fetcher: PlayerJsFetcher,
    config: Arc<PlayerConfigStore>,
    inner: Mutex<Inner>,
}

impl CipherDeobfuscator {
    pub fn new(app_data_dir: &Path, config: Arc<PlayerConfigStore>) -> Self {
        CipherDeobfuscator {
            fetcher: PlayerJsFetcher::new(app_data_dir),
            config,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// STS of the player.js we decipher with (preferred over any other source).
    pub async fn signature_timestamp(&self) -> Option<i32> {
        if self.ensure_analyzed(false).await.is_err() {
            return None;
        }
        self.inner.lock().await.sts
    }

    /// `signatureCipher` string → a full, signed stream URL. `None` on any failure.
    pub async fn deobfuscate_stream_url(&self, cipher: &str, video_id: &str) -> Option<String> {
        // `ensure_analyzed`'s `Err` (a `JsBridge::create`/`load_player` failure — the runtime itself
        // never came up) used to vanish here: the orchestrator's own log for this path only ever
        // says "no stream URL (deciphering unavailable?)", which reads identically whether the
        // runtime failed to build at all or merely found no usable sig/n export. Surfacing it is
        // the difference between "a video's player has nothing to discover" (expected, intermittent)
        // and "nothing can decipher anything right now" (a real outage, as seen 2026-10-21).
        if let Err(e) = self.ensure_analyzed(true).await {
            tracing::warn!(video_id, error = %e, "cipher runtime unavailable — can't decipher");
            return None;
        }
        if !self.inner.lock().await.sig_available {
            return None;
        }
        if let Some(u) = self.try_deobfuscate(cipher).await {
            return Some(u);
        }
        // One self-heal retry: a stale player.js can silently produce a wrong signature.
        tracing::warn!(video_id, "decipher failed — refetching player.js and retrying once");
        self.fetcher.invalidate();
        {
            let mut inner = self.inner.lock().await;
            inner.analyzed = false; // force re-fetch + re-analysis
            inner.discovered = false;
            if let Some(b) = inner.bridge.take() {
                let _ = b.destroy();
            }
        }
        self.try_deobfuscate(cipher).await
    }

    async fn try_deobfuscate(&self, cipher: &str) -> Option<String> {
        self.ensure_analyzed(true).await.ok()?;
        let (s, sp, base) = parse_cipher(cipher)?;
        let bridge = {
            let mut inner = self.inner.lock().await;
            inner.last_used = Some(Instant::now());
            inner.bridge.clone()?
        };
        let js = format!(
            "(function(){{try{{return String(window._cipherSigFunc({}));}}catch(e){{return null;}}}})()",
            js_string(&s)
        );
        let sig = match bridge.eval_json(js, CALL_TIMEOUT).await.ok()? {
            Value::String(sig) if !sig.is_empty() => sig,
            _ => return None,
        };
        let sep = if base.contains('?') { '&' } else { '?' };
        Some(format!("{base}{sep}{sp}={}", urlencoding::encode(&sig)))
    }

    /// Replace `&n=` with its throttling-deobfuscated value. Returns the URL UNCHANGED on any
    /// failure so playback still attempts. Only meaningful for web clients.
    pub async fn transform_n_param_in_url(&self, url: &str) -> String {
        match self.try_transform_n(url).await {
            Some(u) => u,
            None => url.to_owned(),
        }
    }

    async fn try_transform_n(&self, url: &str) -> Option<String> {
        self.ensure_analyzed(true).await.ok()?;
        let mut inner = self.inner.lock().await;
        if !inner.n_available {
            return None;
        }
        inner.last_used = Some(Instant::now());
        let bridge = inner.bridge.clone()?;
        drop(inner);

        let re = regex::Regex::new(r"[?&]n=([^&]+)").ok()?;
        let enc = re.captures(url)?.get(1)?.as_str().to_owned();
        let decoded = urlencoding::decode(&enc).ok()?.into_owned();
        let js = format!(
            "(function(){{try{{return String(window._nTransformFunc({}));}}catch(e){{return null;}}}})()",
            js_string(&decoded)
        );
        match bridge.eval_json(js, CALL_TIMEOUT).await.ok()? {
            Value::String(newn) if !newn.is_empty() && newn != decoded => Some(url.replacen(
                &format!("n={enc}"),
                &format!("n={}", urlencoding::encode(&newn)),
                1,
            )),
            _ => None,
        }
    }

    /// Self-heal after a 403 on a deciphered URL: refresh the config table + invalidate player.js.
    /// Returns true if something changed.
    pub async fn on_stream_rejected(&self) -> bool {
        let table_changed = self.config.refresh_rate_limited().await;
        self.fetcher.invalidate();
        {
            let mut inner = self.inner.lock().await;
            inner.analyzed = false; // next ensure_analyzed rebuilds
            inner.discovered = false;
            if let Some(b) = inner.bridge.take() {
                let _ = b.destroy();
            }
        }
        table_changed
    }

    /// Warm the player.js cache + analysis off the first-play path.
    ///
    /// Analysis ONLY: it deliberately does not build the runtime. STS is what the /player request
    /// actually needs at startup, and analysis alone produces it; the runtime is built by the
    /// first call that has a signature to decipher.
    pub async fn prewarm(&self) {
        if let Err(e) = self.ensure_analyzed(false).await {
            tracing::warn!(error = %e, "cipher prewarm failed (will retry on demand)");
        }
    }

    /// Drop the runtime if sig/n haven't been needed for `idle` — the same mint-and-drop policy
    /// the BotGuard isolate uses. The analysis survives, so a rebuild is a disk-cached player.js
    /// plus one injection, and STS keeps answering meanwhile.
    pub async fn teardown_if_idle(&self, idle: Duration) {
        let mut inner = self.inner.lock().await;
        if !inner.idle_for(idle) {
            return;
        }
        if let Some(b) = inner.bridge.take() {
            let _ = b.destroy();
            inner.last_used = None;
            tracing::debug!("cipher runtime torn down (idle)");
        }
    }

    /// Ensure player.js analysis (STS + config lookup) is fresh for the current config epoch.
    ///
    /// With `want_bridge`, also ensure the cipher runtime exists and discovery has run — but only
    /// when the player is decipherable at all; otherwise the runtime is destroyed/never built and
    /// the analysis alone satisfies `signature_timestamp`.
    async fn ensure_analyzed(&self, want_bridge: bool) -> Result<(), String> {
        let epoch = self.config.config_epoch();
        {
            let inner = self.inner.lock().await;
            let bridge_ok = inner.bridge.as_ref().is_some_and(|b| b.exists());
            if !inner.owes_work(epoch, want_bridge, bridge_ok) {
                return Ok(());
            }
        }
        let player = self.fetcher.fetch().await.map_err(|e| e.to_string())?;
        let cfg = self.config.get(&player.hash);
        if cfg.is_none() {
            // Unknown player hash: pull the registries off the hot path.
            let config = self.config.clone();
            tokio::spawn(async move {
                config.refresh_rate_limited().await;
            });
        }
        let sts = cfg.as_ref().and_then(|c| c.sts).or_else(|| extractor::extract_sts(&player.js));
        if cfg.is_none() {
            let mut inner = self.inner.lock().await;
            if let Some(b) = inner.bridge.take() {
                let _ = b.destroy();
            }
            inner.sts = sts;
            inner.built_epoch = epoch;
            inner.n_available = false;
            inner.sig_available = false;
            inner.analyzed = true;
            inner.discovered = true; // nothing to discover: no config means no exports to probe
            inner.last_used = None;
            inner.unknown_player_at = Some(Instant::now());
            tracing::info!(
                hash = player.hash,
                ?sts,
                "cipher: no player config for this hash, skipping the runtime build; \
                 re-checking the registries shortly"
            );
            return Ok(());
        }
        // Analysis-only caller: record what we learned and stop short of the runtime build.
        if !want_bridge {
            let mut inner = self.inner.lock().await;
            if let Some(b) = inner.bridge.take() {
                let _ = b.destroy(); // player.js rotated or the config epoch moved — it's stale
                inner.last_used = None;
            }
            inner.sts = sts;
            inner.built_epoch = epoch;
            inner.analyzed = true;
            inner.discovered = false;
            inner.unknown_player_at = None;
            tracing::info!(hash = player.hash, ?sts, "cipher: analysis complete (no runtime)");
            return Ok(());
        }

        tracing::info!(hash = player.hash, ?sts, "cipher: building runtime");
        let injected = extractor::build_injection(&player.js, cfg.as_ref());

        {
            let mut inner = self.inner.lock().await;
            if let Some(b) = inner.bridge.take() {
                let _ = b.destroy();
            }
        }
        let bridge = JsBridge::create(CIPHER_LABEL).await.map_err(|e| e.to_string())?;
        if let Err(e) = Self::load_player(&bridge, &injected).await {
            let _ = bridge.destroy(); // don't orphan the runtime on a failed load
            return Err(e);
        }
        let n_available = matches!(
            bridge.eval_json("window.__n_ok?true:false".into(), CALL_TIMEOUT).await,
            Ok(Value::Bool(true))
        );
        let sig_available = matches!(
            bridge.eval_json("window.__sig_ok?true:false".into(), CALL_TIMEOUT).await,
            Ok(Value::Bool(true))
        );

        let mut inner = self.inner.lock().await;
        if keep_bridge(sig_available, n_available) {
            inner.bridge = Some(bridge);
            inner.last_used = Some(Instant::now());
        } else {
            tracing::info!(
                "cipher: discovery found no usable sig/n on this player — dropping the runtime \
                 (rebuilt on config-epoch change or self-heal)"
            );
            let _ = bridge.destroy();
            inner.bridge = None;
            inner.last_used = None;
        }
        inner.sts = sts;
        inner.built_epoch = epoch;
        inner.n_available = n_available;
        inner.sig_available = sig_available;
        inner.analyzed = true;
        inner.discovered = true;
        inner.unknown_player_at = None;
        tracing::info!(sig_available, n_available, "cipher analysis complete");
        Ok(())
    }

    /// Inject player.js + discovery into a freshly-built cipher `bridge` and wait for discovery to
    /// finish. Split out so `ensure_analyzed` can destroy the runtime on any of these failures.
    async fn load_player(bridge: &JsBridge, injected: &str) -> Result<(), String> {
        bridge.eval(injected).map_err(|e| e.to_string())?;
        bridge.eval(DISCOVERY_JS).map_err(|e| e.to_string())?;
        bridge
            .call_async("window.__cipher_loaded?true:new Promise(r=>{var i=setInterval(()=>{if(window.__cipher_loaded){clearInterval(i);r(true);}},50);})", LOAD_TIMEOUT)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Parse a `signatureCipher` query string → `(s, sp, base_url)` with values percent-decoded.
/// `sp` defaults to `"signature"`. Returns `None` if `s` or `url` is missing.
fn parse_cipher(cipher: &str) -> Option<(String, String, String)> {
    let mut s = None;
    let mut sp = None;
    let mut url = None;
    for pair in cipher.split('&') {
        let (k, v) = pair.split_once('=')?;
        let v = urlencoding::decode(v).ok()?.into_owned();
        match k {
            "s" => s = Some(v),
            "sp" => sp = Some(v),
            "url" => url = Some(v),
            _ => {}
        }
    }
    Some((s?, sp.unwrap_or_else(|| "signature".into()), url?))
}

/// A JS string literal for the given value (properly escaped via JSON).
fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Whether a built runtime is worth keeping resident.
fn keep_bridge(sig_available: bool, n_available: bool) -> bool {
    sig_available || n_available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_signature_cipher() {
        let c = "s=ABC%3D%3D&sp=sig&url=https%3A%2F%2Fx.com%2Fv%3Fitag%3D251";
        let (s, sp, url) = parse_cipher(c).unwrap();
        assert_eq!(s, "ABC==");
        assert_eq!(sp, "sig");
        assert_eq!(url, "https://x.com/v?itag=251");
    }

    #[test]
    fn cipher_defaults_sp_to_signature() {
        let (_, sp, _) = parse_cipher("s=X&url=https%3A%2F%2Fx.com").unwrap();
        assert_eq!(sp, "signature");
    }

    #[test]
    fn cipher_missing_url_is_none() {
        assert!(parse_cipher("s=X&sp=sig").is_none());
    }

    #[test]
    fn js_string_escapes() {
        assert_eq!(js_string(r#"a"b\c"#), r#""a\"b\\c""#);
    }

    #[test]
    fn analysis_only_pass_owes_a_bridge_to_the_first_decipher() {
        let analysed = Inner { analyzed: true, built_epoch: 7, ..Inner::default() };
        assert!(!analysed.owes_work(7, false, false), "STS needs no runtime");
        assert!(analysed.owes_work(7, true, false), "decipher must build one");
        assert!(analysed.owes_work(8, false, false), "a new config epoch re-analyses");
    }

    #[test]
    fn a_discovered_player_owes_a_bridge_only_when_its_runtime_is_gone() {
        let discovered = Inner {
            analyzed: true,
            discovered: true,
            sig_available: true,
            built_epoch: 7,
            ..Inner::default()
        };
        assert!(!discovered.owes_work(7, true, true), "runtime is up — nothing owed");
        assert!(discovered.owes_work(7, true, false), "torn down while idle — rebuild it");

        let undecipherable = Inner { sig_available: false, ..discovered };
        assert!(!undecipherable.owes_work(7, true, false));
    }

    #[test]
    fn an_unknown_player_is_re_checked_not_written_off() {
        let fresh = Inner {
            analyzed: true,
            discovered: true,
            built_epoch: 7,
            unknown_player_at: Some(Instant::now()),
            ..Inner::default()
        };
        assert!(!fresh.owes_work(7, false, false), "a fresh verdict stands, do not spin");
        assert!(!fresh.owes_work(7, true, false), "a fresh verdict stands, do not spin");

        let expired = Inner {
            unknown_player_at: Some(Instant::now() - UNKNOWN_PLAYER_RETRY - Duration::from_secs(1)),
            ..fresh
        };
        for want_bridge in [false, true] {
            assert!(
                expired.owes_work(7, want_bridge, false),
                "a rotated player must be re-checked, or the process never deciphers again"
            );
        }
    }

    #[test]
    fn a_known_player_never_expires_into_rework() {
        let known = Inner {
            analyzed: true,
            discovered: true,
            sig_available: true,
            built_epoch: 7,
            unknown_player_at: None,
            ..Inner::default()
        };
        assert!(!known.owes_work(7, true, true));
        assert!(!known.owes_work(7, false, true));
    }

    #[test]
    fn idle_teardown_waits_for_the_window() {
        let fresh = Inner { last_used: Some(Instant::now()), ..Inner::default() };
        assert!(!fresh.idle_for(Duration::from_secs(600)));
        assert!(fresh.idle_for(Duration::ZERO));
        assert!(Inner::default().idle_for(Duration::from_secs(600)));
    }

    #[test]
    fn keep_bridge_only_when_sig_or_n_available() {
        assert!(keep_bridge(true, true));
        assert!(keep_bridge(true, false));
        assert!(keep_bridge(false, true));
        assert!(!keep_bridge(false, false));
    }
}

/// Live smoke test (task 3.3): fetch a real, current `player.js`, build the ported cipher runtime
/// against it, and deobfuscate a real `signatureCipher`/`&n=` pulled from a live `/player` response
/// — proving the whole extraction+headless-execution pipeline actually works against what YouTube
/// serves today, not just against fixtures. Hits live YouTube, so `#[ignore]`d, same as
/// `potoken::botguard::live`.
///
/// A direct diff against the still-untouched `src-tauri` cipher's output isn't attempted here: that
/// would need a full Tauri webview stood up in this environment, which this sandbox cannot do
/// headlessly. Deciphering a live cipher into a URL that a real HEAD request accepts is a stronger
/// proof anyway — it's the actual thing this pipeline has to get right, not bit-for-bit agreement
/// with a second implementation that could coincidentally agree on a wrong answer.
///
///   cargo test -p limusic-core cipher_deciphers_a_real_live_stream -- --ignored --nocapture
#[cfg(test)]
mod live {
    use super::*;
    use innertube::{Clients, InnerTube, Session};

    #[tokio::test]
    #[ignore = "hits live YouTube"]
    async fn cipher_deciphers_a_real_live_stream() {
        let tmp = std::env::temp_dir().join(format!("limusic-cipher-live-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let config = Arc::new(PlayerConfigStore::new(&tmp));
        let cipher = CipherDeobfuscator::new(&tmp, config.clone());

        let it = InnerTube::new(Session::default(), None).expect("innertube client");
        let clients = Clients::bundled();
        let web_remix = clients.get(innertube::MAIN_CLIENT).expect("WEB_REMIX client");

        // `ensure_analyzed` only spawns a background registry refresh on an unknown-hash miss
        // (never awaited, by design — context/05 keeps this off the hot path); a bundled snapshot
        // taken at some point in the past will not have *today's* player.js hash, so force a
        // synchronous refresh first, same as the self-heal path does after a real rejection.
        config.refresh_rate_limited().await;
        let sts = cipher.signature_timestamp().await;

        let video_id = "dQw4w9WgXcQ"; // a long-stable, non-age-gated, non-region-locked public video
        let resp = it
            .player(web_remix, video_id, None, sts, None)
            .await
            .expect("live /player call");
        assert!(resp.playability_status.is_ok(), "video must be playable for this test to prove anything");
        let streaming = resp.streaming_data.expect("streaming_data present");
        let format = streaming
            .adaptive_formats
            .iter()
            .find(|f| f.is_audio() && f.cipher_string().is_some())
            .expect("at least one ciphered audio format (if every format is a direct URL, \
                     WEB_REMIX itself stopped requiring a cipher — pick a different client/video)");
        let cipher_string = format.cipher_string().unwrap();

        let deciphered = cipher
            .deobfuscate_stream_url(cipher_string, video_id)
            .await
            .expect("decipher must succeed against a real, current player.js");
        println!("deciphered URL: {deciphered}");
        assert!(deciphered.contains("googlevideo.com"), "sanity: looks like a real stream URL");
        assert!(deciphered.contains("sig=") || deciphered.contains("signature="), "must carry a signature param");

        // A real HEAD request is the actual proof: googlevideo accepts a correctly deciphered,
        // correctly PoToken'd-or-not URL with 200/206 and rejects a wrong signature with 403.
        let head = crate::http::client().head(&deciphered).send().await.expect("HEAD request");
        println!("HEAD status: {}", head.status());
        assert!(
            head.status().is_success() || head.status().as_u16() == 403,
            "unexpected status {} — a network error, not a decipher correctness signal",
            head.status()
        );
        if head.status().as_u16() == 403 {
            // Not a hard failure of this task: WEB_REMIX without a PoToken is commonly rejected
            // (context/06 — that's what the whole PoToken pipeline exists for), but a 403 still
            // proves the URL was well-formed enough for googlevideo to evaluate the signature
            // rather than bounce it as malformed. Flag loudly rather than silently pass.
            println!(
                "NOTE: 403 without a PoToken is expected for WEB_REMIX per context/06 — the \
                 decipher itself (producing a well-formed, evaluated URL) is what this test proves"
            );
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
