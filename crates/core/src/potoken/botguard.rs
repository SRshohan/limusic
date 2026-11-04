//! BotGuard minting, driven on the shared v8 thread (`crate::v8root`), under `rustypipe-botguard`
//! (deno_core + JSDOM).
//!
//! Ported from `src-tauri/src/potoken/botguard.rs` (openspec/changes/http-audio-streaming). The
//! only structural change from that file: the thread-spawning `OnceLock<mpsc::UnboundedSender<Cmd>>`
//! singleton (`host()`) is gone — this module's [`State`]/[`handle`] are driven by
//! `crate::v8root`'s shared loop instead of owning a thread of their own (design.md D1).
//!
//! **No length-based class re-roll any more.** This used to bootstrap up to 8 times hunting for an
//! integrity-token byte length believed to mark which of two acceptance classes googlevideo
//! honoured, re-rolling until the pot's own length looked right before it was ever attached to a
//! URL. Dropped after two things pointed the same direction at once: every bootstrap in this app's
//! own production logs was landing in the "rejected" class regardless of how many times it
//! re-rolled (not occasional bad luck — consistently, across days), and `rustypipe-botguard`
//! upstream reached the same conclusion from the inside, removing their *own* internal PO-token
//! length check in v0.1.2+ (codeberg.org/ThetaDev/rustypipe-botguard commit 3cf61bb, "fix: remove
//! PO token length check") because length alone isn't a reliable validity signal. Trusting whatever
//! `rustypipe-botguard` itself accepts is both simpler and more likely correct than re-deriving a
//! now-stale byte threshold ourselves. Also stopped overriding the botguard runtime's user agent:
//! upstream's own default (`DEFAULT_UA`, a WebKit/Safari string) is what they maintain and test
//! against — this app was passing a Chrome UA instead, which upstream's own doc comment says
//! produces invalid tokens ("this has to be a Webkit user agent, using other user agents results in
//! invalid tokens"). That's unrelated to the separate, legitimate Chrome identity the actual
//! `/player` requests use (`WEB_REMIX` et al. in `crates/innertube`) — the botguard runtime's UA
//! only shapes the JS environment BotGuard's own challenge executes in, not anything the outer HTTP
//! request later claims to be.
//!
//! `deno_core::JsRuntime` (inside `rustypipe_botguard::Botguard`) is `!Send`, so it must be built
//! and used entirely on the v8 thread, never held across an `.await` that could resume elsewhere —
//! `crate::v8root` is what guarantees that thread is the only one in the process touching V8 at
//! all.

use rustypipe_botguard::{Botguard, Error as BgError};
use tokio::sync::oneshot;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No JS runtime can be stood up at all: the thread would not spawn, or tokio would not build.
    /// Nothing will fix that at runtime, so the caller latches and degrades permanently.
    #[error("botguard runtime: {0}")]
    Fatal(String),
    /// Network trouble, or a challenge we could not solve. Worth trying again on the next track.
    #[error("botguard: {0}")]
    Transient(String),
}

/// Everything BotGuard itself can fail at is transient, `BgError::Js` included: a JS error here
/// means the program YouTube shipped today threw, which is the case that fixes itself the next
/// time the program is downloaded.
fn classify(e: BgError) -> Error {
    Error::Transient(e.to_string())
}

pub(crate) enum Cmd {
    /// Build a fresh runtime, replacing whatever is live. Replies with its generation, the session
    /// token it minted on the way, and the integrity token's lifetime.
    Bootstrap {
        session_ident: String,
        reply: oneshot::Sender<Result<(u64, String, u64), Error>>,
    },
    Mint {
        generation: u64,
        ident: String,
        reply: oneshot::Sender<Result<String, Error>>,
    },
    /// Free the isolate, if `generation` is still the live one.
    Shutdown {
        generation: u64,
    },
    /// Thread-identity probe (`crate::v8root`'s own test suite) — proves BotGuard and cipher jobs
    /// are served on the same OS thread, which is the property design.md D1 depends on.
    #[cfg(test)]
    Probe(oneshot::Sender<std::thread::ThreadId>),
}

/// This module's share of the v8 thread's persistent state — lives entirely inside
/// `crate::v8root`'s loop, never touched from any other thread.
#[derive(Default)]
pub(crate) struct State {
    generation: u64,
    live: Option<(u64, Botguard)>,
}

/// Handle one [`Cmd`] against this module's slice of the shared v8 thread's state.
pub(crate) async fn handle(state: &mut State, cmd: Cmd) {
    match cmd {
        Cmd::Bootstrap { session_ident, reply } => {
            // Never two isolates at once: v8 asserts they are dropped in reverse creation order
            // (v8-130 isolate.rs:1666) and panics this thread if they overlap.
            drop(state.live.take());
            state.generation += 1;
            match bootstrap(&session_ident).await {
                Ok((bg, token, lifetime)) => {
                    // A failed send means the caller timed out and will never hold this
                    // generation, so `bg` drops here rather than idling.
                    if reply.send(Ok((state.generation, token, lifetime))).is_ok() {
                        state.live = Some((state.generation, bg));
                    }
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            }
        }
        Cmd::Mint { generation, ident, reply } => {
            let minted = match &mut state.live {
                Some((live_gen, bg)) if *live_gen == generation => {
                    bg.mint_token(&ident).await.map_err(classify)
                }
                _ => Err(Error::Transient("botguard runtime is gone".into())),
            };
            let _ = reply.send(minted);
        }
        Cmd::Shutdown { generation } => {
            if state.live.as_ref().is_some_and(|(live_gen, _)| *live_gen == generation) {
                state.live = None;
            }
        }
        #[cfg(test)]
        Cmd::Probe(reply) => {
            let _ = reply.send(std::thread::current().id());
        }
    }
}

/// A live BotGuard runtime. Clone-free on purpose: one owner, and dropping it frees the isolate.
pub struct Minter {
    generation: u64,
}

impl Drop for Minter {
    fn drop(&mut self) {
        let _ = crate::v8root::send(crate::v8root::Job::BotGuard(Cmd::Shutdown {
            generation: self.generation,
        }));
    }
}

/// What a successful bootstrap yields. The session token comes out of the bootstrap because the
/// class check needs a real mint anyway, so it costs nothing to keep it.
pub struct Bootstrap {
    pub minter: Minter,
    pub session_token: String,
    pub lifetime_secs: u64,
}

impl Minter {
    /// Stand up a BotGuard runtime bound to `session_ident` (visitorData).
    pub async fn spawn(session_ident: String) -> Result<Bootstrap, Error> {
        let (reply, rx) = oneshot::channel();
        crate::v8root::send(crate::v8root::Job::BotGuard(Cmd::Bootstrap { session_ident, reply }))
            .map_err(|_| Error::Fatal("v8 thread unavailable".into()))?;
        let (generation, session_token, lifetime_secs) = rx
            .await
            .map_err(|_| Error::Fatal("v8 thread stopped during bootstrap".into()))??;
        Ok(Bootstrap { minter: Minter { generation }, session_token, lifetime_secs })
    }

    /// Mint one PoToken for `ident` (a videoId, for the `&pot=` URL parameter).
    pub async fn mint(&self, ident: &str) -> Result<String, Error> {
        let (reply, rx) = oneshot::channel();
        crate::v8root::send(crate::v8root::Job::BotGuard(Cmd::Mint {
            generation: self.generation,
            ident: ident.to_owned(),
            reply,
        }))
        .map_err(|_| Error::Transient("v8 thread gone".into()))?;
        rx.await.map_err(|_| Error::Transient("v8 thread gone".into()))?
    }
}

/// `Botguard::init` + one session mint. No `user_agent` override (see this module's header
/// comment) — `Botguard::builder()`'s own default is upstream's maintained WebKit UA.
///
/// No snapshot path is passed: `rustypipe-botguard` caches a solved runtime in a process-wide
/// `OnceLock`, so a snapshot would freeze in whatever shape it was written with for the rest of
/// the process.
async fn bootstrap(session_ident: &str) -> Result<(Botguard, String, u64), Error> {
    let mut bg = Botguard::builder().init().await.map_err(classify)?;
    // Session token first, on a fresh minter, before any other identifier — Metrolist enforces
    // that ordering under a mutex and there is no reason to find out the hard way why.
    let token = bg.mint_token(session_ident).await.map_err(classify)?;
    let lifetime = u64::from(bg.lifetime());
    Ok((bg, token, lifetime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_errors_are_transient() {
        assert!(matches!(classify(BgError::Js("boom".into())), Error::Transient(_)));
        assert!(matches!(classify(BgError::InvalidPoToken("nope".into())), Error::Transient(_)));
    }

    /// Two jobs through `handle`, driven directly (not via the real v8 thread) — the fast, V8-free
    /// half of the port verification. Thread identity is the other half, proven in `crate::v8root`'s
    /// own test using `Cmd::Probe`.
    #[tokio::test]
    async fn shutdown_is_a_no_op_for_a_generation_nothing_holds() {
        let mut state = State::default();
        handle(&mut state, Cmd::Shutdown { generation: 7 }).await;
        assert!(state.live.is_none());
    }

    #[tokio::test]
    async fn mint_against_an_unknown_generation_is_transient() {
        let mut state = State::default();
        let (reply, rx) = oneshot::channel();
        handle(&mut state, Cmd::Mint { generation: 999, ident: "x".into(), reply }).await;
        assert!(matches!(rx.await.unwrap(), Err(Error::Transient(_))));
    }
}

/// Live smoke test: stand up a real BotGuard runtime and mint a real pot. Hits YouTube, so it is
/// `#[ignore]`d and never runs in the default suite.
///
///   cargo test -p limusic-core botguard_mints_a_token -- --ignored --nocapture
#[cfg(test)]
mod live {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    #[ignore = "hits live YouTube"]
    async fn botguard_mints_a_token() {
        let visitor = std::env::var("LIMUSIC_VISITOR_DATA")
            .expect("set LIMUSIC_VISITOR_DATA to the app's visitor_data setting");
        let video_id = std::env::var("LIMUSIC_VIDEO_ID").unwrap_or_else(|_| "PtHEr7siapo".into());

        let t0 = Instant::now();
        let b = Minter::spawn(visitor.clone()).await.expect("bootstrap");
        let boot = t0.elapsed();
        let t1 = Instant::now();
        let pot = b.minter.mint(&video_id).await.expect("mint");
        println!(
            "bootstrap {boot:?} (ttl {}s) | mint {:?}\npot {pot}",
            b.lifetime_secs,
            t1.elapsed()
        );

        // The idle teardown drops the minter, which frees the V8 isolate; the next track start
        // builds a fresh one on the same thread. This is the #133 crash: before the singleton
        // thread, the second isolate died with SIGSEGV / SEGV_PKUERR.
        drop(b);
        let again = Minter::spawn(visitor).await.expect("second bootstrap after teardown");
        let pot2 = again.minter.mint(&video_id).await.expect("mint after teardown");
        println!("after teardown: pot {pot2}");
    }
}
