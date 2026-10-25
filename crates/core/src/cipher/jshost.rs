//! Headless cipher JS host (openspec/changes/http-audio-streaming, design.md D3) — a `deno_core`-
//! backed replacement for `src-tauri/src/webview.rs`'s hidden-webview `Bridge`, same method shape
//! (`create`/`eval`/`eval_json`/`call_async`/`destroy`/`exists`) minus the `AppHandle` param.
//! Everything runs on the shared v8 thread (`crate::v8root`) — never a second one, see its header
//! comment for why that matters.
//!
//! **Correction found during task 3.3's live verification, not assumed from reading our own code
//! alone**: design.md D3 originally claimed no real DOM was needed, reasoning from the fact that
//! `cipher/*.rs` itself (the Rust orchestration code) never touches `document`/`fetch`/
//! `window.location`. That check never covered the *content* of `player.js` — a multi-megabyte,
//! externally-sourced bundle we don't control — and its top-level module code (unrelated to the
//! sig/n functions we actually call) turned out to reference `document`, `XMLHttpRequest`, and
//! several other browser globals during initialization, well before reaching the functions we
//! need. A bare `deno_core::JsRuntime` with truly nothing defined throws a `ReferenceError` partway
//! through evaluating the whole file, so `window._cipherSigFunc`/`_nTransformFunc` never even get
//! assigned. Fixed by giving the harness a deliberately shallow shim layer — `document`/
//! `navigator`/`location`/`performance`/`XMLHttpRequest`/`fetch`/timers/storage/observers/etc. as
//! inert stand-ins, just enough for player.js's startup code to run to completion without
//! crashing. None of this needs to *work* correctly (the cipher never calls into it), it only needs
//! to exist so references to it don't throw. This is still far short of `deno_web`'s full JSDOM-ish
//! extension (confirmed live: the shim above was sufficient, no snapshot-probing concerns like
//! BotGuard's apply here), so the "no extensions" design (vs. BotGuard's `deno_web`-based globals)
//! still holds — just with a wider, hand-written shim instead of none at all. `window` itself
//! doesn't exist on a bare runtime either, so the harness aliases it to `globalThis` first.
//!
//! `eval_json`/`call_async`'s synchronous-value round trip is actually simpler than the webview
//! version: `execute_script` returns a thrown JS exception directly as an `Err`, so the original
//! harness's `window.onerror`/`window.onunhandledrejection` → `__jserr` indirection (needed because
//! `WebviewWindow::eval_with_callback` didn't propagate script-level exceptions) is dropped
//! entirely — there is nothing left for it to work around. `call_async`'s poll-the-global-slot
//! mechanism is kept unchanged even so (design.md D3): it's what makes an async (promise-returning)
//! JS expression bridge back to Rust at all, and proving behavioral equivalence with the webview
//! version comes first; native-promise simplification is later cleanup.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use deno_core::{JsRuntime, PollEventLoopOptions, RuntimeOptions};
use serde_json::Value;
use tokio::sync::oneshot;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cipher runtime is gone")]
    Gone,
    #[error("cipher eval failed: {0}")]
    Eval(String),
    #[error("timed out after {0:?}")]
    Timeout(Duration),
    #[error("build failed: {0}")]
    Build(String),
}

/// The bare globals `player.js`'s IIFE expects, aliasing `window` to `globalThis` (a bare
/// `JsRuntime` has no `window` of its own) and seeding `__slots`, the bag [`JsBridge::call_async`]
/// parks results in — ported from `webview.rs`'s `HARNESS_HTML` inline script, minus the HTML
/// wrapper and the error-hook globals (see module doc: no longer needed).
const HARNESS_JS: &str = "\
globalThis.window = globalThis;\
window.__slots = {};\
window._yt_player = window._yt_player || {};\
window.document = window.document || { \
  createElement: function(){ return { style: {}, setAttribute: function(){}, appendChild: function(){} }; }, \
  documentElement: { style: {} }, \
  addEventListener: function(){}, \
  getElementsByTagName: function(){ return []; }, \
  location: { href: '', protocol: 'https:' } \
};\
window.navigator = window.navigator || { userAgent: '', language: 'en-US', platform: '', onLine: true };\
window.location = window.location || { href: 'https://www.youtube.com/', protocol: 'https:', hostname: 'www.youtube.com', search: '' };\
window.performance = window.performance || { now: function(){ return 0; }, mark: function(){}, measure: function(){} };\
window.addEventListener = window.addEventListener || function(){};\
window.removeEventListener = window.removeEventListener || function(){};\
window.dispatchEvent = window.dispatchEvent || function(){ return true; };\
window.setTimeout = window.setTimeout || function(fn){ if (typeof fn === 'function') fn(); return 0; };\
window.clearTimeout = window.clearTimeout || function(){};\
window.setInterval = window.setInterval || function(){ return 0; };\
window.clearInterval = window.clearInterval || function(){};\
window.requestAnimationFrame = window.requestAnimationFrame || function(){ return 0; };\
window.cancelAnimationFrame = window.cancelAnimationFrame || function(){};\
function StubXHR(){}\
StubXHR.prototype.open = function(){};\
StubXHR.prototype.send = function(){};\
StubXHR.prototype.setRequestHeader = function(){};\
StubXHR.prototype.addEventListener = function(){};\
window.XMLHttpRequest = window.XMLHttpRequest || StubXHR;\
window.fetch = window.fetch || function(){ return Promise.reject(new Error('fetch unavailable in cipher runtime')); };\
window.WebSocket = window.WebSocket || function(){};\
window.localStorage = window.localStorage || { getItem: function(){ return null; }, setItem: function(){}, removeItem: function(){} };\
window.sessionStorage = window.sessionStorage || window.localStorage;\
window.screen = window.screen || { width: 1920, height: 1080 };\
window.history = window.history || { pushState: function(){}, replaceState: function(){} };\
window.MutationObserver = window.MutationObserver || function(){ this.observe = function(){}; this.disconnect = function(){}; };\
window.IntersectionObserver = window.IntersectionObserver || window.MutationObserver;\
window.ResizeObserver = window.ResizeObserver || window.MutationObserver;\
window.CustomEvent = window.CustomEvent || function(type){ this.type = type; };\
window.Image = window.Image || function(){};\
window.Audio = window.Audio || function(){};\
";

pub(crate) enum Cmd {
    /// Build a fresh runtime, replacing whatever is live.
    Create { reply: oneshot::Sender<Result<(u64, Arc<AtomicBool>), Error>> },
    /// Fire-and-forget (no reply, matching the original `Bridge::eval`'s synchronous signature —
    /// enqueuing onto the v8 thread's channel is itself non-blocking).
    Eval { generation: u64, js: String },
    EvalJson { generation: u64, js: String, reply: oneshot::Sender<Result<Value, Error>> },
    Destroy { generation: u64 },
    /// Thread-identity probe (`crate::v8root`'s own test suite).
    #[cfg(test)]
    Probe(oneshot::Sender<std::thread::ThreadId>),
    /// Deliberately panics `handle()` (`crate::v8root`'s own test suite) — proves one job's panic
    /// cannot take the shared v8 thread down with it.
    #[cfg(test)]
    Panic,
}

/// This module's share of the v8 thread's persistent state.
#[derive(Default)]
pub(crate) struct State {
    live: Option<(u64, JsRuntime, Arc<AtomicBool>)>,
    generation: u64,
}

pub(crate) async fn handle(state: &mut State, cmd: Cmd) {
    match cmd {
        Cmd::Create { reply } => {
            if let Some((_, _, alive)) = state.live.take() {
                alive.store(false, Ordering::SeqCst);
            }
            state.generation += 1;
            let gen = state.generation;
            match build_runtime() {
                Ok(rt) => {
                    let alive = Arc::new(AtomicBool::new(true));
                    if reply.send(Ok((gen, alive.clone()))).is_ok() {
                        state.live = Some((gen, rt, alive));
                    } else {
                        alive.store(false, Ordering::SeqCst);
                    }
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            }
        }
        Cmd::Eval { generation, js } => {
            if let Err(e) = run_eval(state, generation, &js).await {
                tracing::debug!(error = ?e, "cipher: fire-and-forget eval failed");
            }
        }
        Cmd::EvalJson { generation, js, reply } => {
            let result = run_eval(state, generation, &js).await;
            let _ = reply.send(result);
        }
        Cmd::Destroy { generation } => {
            if state.live.as_ref().is_some_and(|(g, _, _)| *g == generation) {
                let (_, _, alive) = state.live.take().unwrap();
                alive.store(false, Ordering::SeqCst);
            }
        }
        #[cfg(test)]
        Cmd::Probe(reply) => {
            let _ = reply.send(std::thread::current().id());
        }
        #[cfg(test)]
        Cmd::Panic => panic!("deliberate test panic"),
    }
}

fn build_runtime() -> Result<JsRuntime, Error> {
    let mut rt = JsRuntime::new(RuntimeOptions::default());
    rt.execute_script("harness.js", HARNESS_JS).map_err(|e| Error::Build(e.to_string()))?;
    Ok(rt)
}

async fn run_eval(state: &mut State, generation: u64, js: &str) -> Result<Value, Error> {
    let Some((live_gen, rt, _)) = &mut state.live else {
        return Err(Error::Gone);
    };
    if *live_gen != generation {
        return Err(Error::Gone);
    }
    let global = rt
        .execute_script("eval.js", js.to_owned())
        .map_err(|e| Error::Eval(e.to_string()))?;
    // A bare JsRuntime does not auto-drain microtasks the way a browser's event loop does, so an
    // `await` inside `call_async`'s kicked IIFE needs this to actually store its result into
    // `__slots` before the next poll reads it. Resolves immediately when nothing is pending.
    let _ = rt.run_event_loop(PollEventLoopOptions::default()).await;
    let scope = &mut rt.handle_scope();
    let local = deno_core::v8::Local::new(scope, global);
    serde_v8::from_v8(scope, local).map_err(|e| Error::Eval(e.to_string()))
}

static CALL_ID: AtomicU64 = AtomicU64::new(0);
fn next_call_id() -> u64 {
    CALL_ID.fetch_add(1, Ordering::Relaxed)
}

/// A handle to the one cipher runtime on the shared v8 thread. `Clone`, like the original `Bridge`
/// — cheap (a generation number and a shared alive flag), not the runtime itself.
#[derive(Clone)]
pub struct JsBridge {
    generation: u64,
    alive: Arc<AtomicBool>,
}

impl JsBridge {
    /// Create a fresh cipher runtime, replacing any existing one. `_label` is accepted (unused) to
    /// keep the exact call shape `cipher/mod.rs` already uses against the webview `Bridge`.
    pub async fn create(_label: &str) -> Result<JsBridge, Error> {
        let (reply, rx) = oneshot::channel();
        crate::v8root::send(crate::v8root::Job::Cipher(Cmd::Create { reply }))
            .map_err(|_| Error::Build("v8 thread unavailable".into()))?;
        let (generation, alive) =
            rx.await.map_err(|_| Error::Build("v8 thread stopped during create".into()))??;
        Ok(JsBridge { generation, alive })
    }

    /// True if this runtime is still the live one (not replaced or destroyed).
    pub fn exists(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Fire-and-forget JS (no result awaited).
    pub fn eval(&self, js: &str) -> Result<(), Error> {
        crate::v8root::send(crate::v8root::Job::Cipher(Cmd::Eval {
            generation: self.generation,
            js: js.to_owned(),
        }))
        .map_err(|_| Error::Gone)
    }

    /// Evaluate a synchronous JS expression and return its value as JSON. `Value::Null` for
    /// null/undefined.
    pub async fn eval_json(&self, js: String, timeout: Duration) -> Result<Value, Error> {
        let (reply, rx) = oneshot::channel();
        crate::v8root::send(crate::v8root::Job::Cipher(Cmd::EvalJson {
            generation: self.generation,
            js,
            reply,
        }))
        .map_err(|_| Error::Gone)?;
        tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| Error::Timeout(timeout))?
            .map_err(|_| Error::Eval("callback dropped".into()))?
    }

    /// Poll a synchronous JS expression until it evaluates non-null, or time out.
    async fn poll_json(&self, expr: &str, timeout: Duration) -> Result<Value, Error> {
        let deadline = Instant::now() + timeout;
        let step = Duration::from_millis(100);
        loop {
            match self.eval_json(format!("({expr})"), Duration::from_secs(3)).await {
                Ok(v) if !v.is_null() => return Ok(v),
                Ok(_) => {}           // null → still pending
                Err(Error::Gone) => return Err(Error::Gone),
                Err(_) => {} // a transient eval timeout/error → keep polling until the deadline
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout(timeout));
            }
            tokio::time::sleep(step).await;
        }
    }

    /// Run an **async** JS expression: `await (expr)`, capturing the resolved value or error into
    /// a slot we then poll. This is how a promise-returning expression bridges back to Rust.
    pub async fn call_async(&self, expr: &str, timeout: Duration) -> Result<Value, Error> {
        let id = next_call_id();
        let kick = format!(
            "(async()=>{{try{{window.__slots['{id}']={{ok:1,v:await ({expr})}};}}\
             catch(e){{window.__slots['{id}']={{ok:0,e:String((e&&e.message)||e)}};}}}})();"
        );
        self.eval(&kick)?;
        let res = self.poll_json(&format!("window.__slots['{id}']||null"), timeout).await;
        let _ = self.eval(&format!("delete window.__slots['{id}'];"));
        let slot = res?;
        if slot.get("ok").and_then(Value::as_i64) == Some(1) {
            Ok(slot.get("v").cloned().unwrap_or(Value::Null))
        } else {
            Err(Error::Eval(
                slot.get("e").and_then(Value::as_str).unwrap_or("async call failed").to_owned(),
            ))
        }
    }

    /// Destroy the runtime. Idempotent.
    pub fn destroy(&self) -> Result<(), Error> {
        crate::v8root::send(crate::v8root::Job::Cipher(Cmd::Destroy {
            generation: self.generation,
        }))
        .map_err(|_| Error::Gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `crate::v8root` is a process-wide singleton (by design — one shared v8 thread, design.md
    /// D1), so these tests race against each other's `create()`/`destroy()` under cargo test's
    /// default parallel runner unless serialized. A `tokio::sync::Mutex` (not `std::sync::Mutex`,
    /// to stay `Send`-safe held across an `.await`) does that without a new dependency.
    static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn eval_json_runs_synchronous_js() {
        let _guard = TEST_LOCK.lock().await;
        let bridge = JsBridge::create("test").await.expect("create");
        let v = bridge.eval_json("1+2".into(), Duration::from_secs(3)).await.unwrap();
        assert_eq!(v, serde_json::json!(3));
    }

    #[tokio::test]
    async fn eval_json_reports_thrown_errors() {
        let _guard = TEST_LOCK.lock().await;
        let bridge = JsBridge::create("test").await.expect("create");
        let err = bridge.eval_json("throw new Error('boom')".into(), Duration::from_secs(3)).await;
        assert!(matches!(err, Err(Error::Eval(_))));
    }

    #[tokio::test]
    async fn call_async_resolves_a_synchronous_await() {
        let _guard = TEST_LOCK.lock().await;
        let bridge = JsBridge::create("test").await.expect("create");
        bridge.eval("window._cipherSigFunc = function(s){ return s.split('').reverse().join(''); };").unwrap();
        let v = bridge.call_async("window._cipherSigFunc('abc')", Duration::from_secs(3)).await.unwrap();
        assert_eq!(v, serde_json::json!("cba"));
    }

    #[tokio::test]
    async fn call_async_surfaces_a_thrown_error() {
        let _guard = TEST_LOCK.lock().await;
        let bridge = JsBridge::create("test").await.expect("create");
        let err = bridge.call_async("(function(){throw new Error('nope')})()", Duration::from_secs(3)).await;
        assert!(matches!(err, Err(Error::Eval(ref m)) if m == "nope"));
    }

    #[tokio::test]
    async fn destroy_makes_the_bridge_report_gone() {
        let _guard = TEST_LOCK.lock().await;
        let bridge = JsBridge::create("test").await.expect("create");
        assert!(bridge.exists());
        bridge.destroy().unwrap();
        // `destroy` is async on the v8 thread; give it a moment.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!bridge.exists());
        assert!(matches!(
            bridge.eval_json("1".into(), Duration::from_secs(1)).await,
            Err(Error::Gone)
        ));
    }

    /// A second `create()` replaces the first; the first bridge's handle must then report gone.
    #[tokio::test]
    async fn a_new_create_retires_the_previous_generation() {
        let _guard = TEST_LOCK.lock().await;
        let first = JsBridge::create("test").await.expect("create");
        assert!(first.exists());
        let _second = JsBridge::create("test").await.expect("create");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!first.exists());
    }
}
