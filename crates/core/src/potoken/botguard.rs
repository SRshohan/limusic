//! BotGuard minting, driven on the shared v8 thread (`crate::v8root`), under `rustypipe-botguard`
//! (deno_core + JSDOM).
//!
//! Ported from `src-tauri/src/potoken/botguard.rs` (openspec/changes/http-audio-streaming). The
//! only structural change from that file: the thread-spawning `OnceLock<mpsc::UnboundedSender<Cmd>>`
//! singleton (`host()`) is gone — this module's [`State`]/[`handle`] are driven by
//! `crate::v8root`'s shared loop instead of owning a thread of their own (design.md D1). The
//! bootstrap/mint/shutdown semantics, the class-probe re-roll logic, and the integrity-token-length
//! recovery are otherwise unchanged.
//!
//! **Not a webview, deliberately.** googlevideo honours the pots built from a ~62-byte
//! `/GenerateIT` integrity token and rejects the ones built from a 65-66 byte token, and which
//! class you get is decided by the BotGuard snapshot. A minimal-DOM runtime lands in the accepted
//! class roughly one bootstrap in three; every real browser engine measured 0 in 25. So we mint
//! here instead, and re-roll the bootstrap until the class is right, which the pot's own length
//! tells us before it is ever attached to a URL.
//!
//! `deno_core::JsRuntime` (inside `rustypipe_botguard::Botguard`) is `!Send`, so it must be built
//! and used entirely on the v8 thread, never held across an `.await` that could resume elsewhere —
//! `crate::v8root` is what guarantees that thread is the only one in the process touching V8 at
//! all.

use base64::Engine;
use rustypipe_botguard::{Botguard, Error as BgError};
use tokio::sync::oneshot;

/// Bytes the minter wraps around `identifier + integrity token`. Measured with identifiers 11 and
/// ~520 bytes long: `pot_len == ident_len + integrity_token_len + 14`, in both token classes.
const POT_OVERHEAD: usize = 14;
/// Largest integrity token whose pots googlevideo accepts. The two observed classes are 61-62
/// (accepted) and 65-66 (rejected) with nothing in between, so the threshold sits in the gap.
const ACCEPTED_MAX_IT: usize = 63;
/// How many bootstraps to spend looking for the accepted class. At the measured ~1-in-3 rate this
/// misses about once in 25 launches; a miss keeps the last runtime and mints from it anyway.
const MAX_BOOTSTRAPS: usize = 8;
/// Identifier the class check mints against: plain ASCII, so its decoded byte length is its char
/// length and the integrity-token arithmetic is exact. visitorData is not usable for this — it
/// carries percent escapes, and reading the class off it came out a byte adrift of the truth.
const CLASS_PROBE: &str = "limusicprobe";

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
        user_agent: String,
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
        Cmd::Bootstrap { user_agent, session_ident, reply } => {
            // Never two isolates at once: v8 asserts they are dropped in reverse creation order
            // (v8-130 isolate.rs:1666) and panics this thread if they overlap.
            drop(state.live.take());
            state.generation += 1;
            match bootstrap(&user_agent, &session_ident).await {
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
    /// Stand up a BotGuard runtime bound to `session_ident` (visitorData), re-rolling the whole
    /// bootstrap while it lands in the rejected integrity-token class (see [`bootstrap`]).
    pub async fn spawn(user_agent: String, session_ident: String) -> Result<Bootstrap, Error> {
        let (reply, rx) = oneshot::channel();
        crate::v8root::send(crate::v8root::Job::BotGuard(Cmd::Bootstrap {
            user_agent,
            session_ident,
            reply,
        }))
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

/// `Botguard::init` + one session mint, repeated until the integrity token is in the class
/// googlevideo honours, or `MAX_BOOTSTRAPS` tries in which case the last runtime is used anyway.
///
/// No snapshot path is passed. `rustypipe-botguard` caches a solved runtime in a process-wide
/// `OnceLock`, so a snapshot would freeze whichever class it was written with for the rest of the
/// process and make this loop unable to re-roll.
async fn bootstrap(
    user_agent: &str,
    session_ident: &str,
) -> Result<(Botguard, String, u64), Error> {
    let mut last = None;
    for attempt in 1..=MAX_BOOTSTRAPS {
        // Drop the previous attempt's runtime before building the next: v8 asserts isolates are
        // dropped in reverse creation order (v8-130 isolate.rs:1666) and panics the whole thread
        // if two overlap, which kills the minter for the rest of the process.
        drop(last.take());
        let mut bg = Botguard::builder().user_agent(user_agent).init().await.map_err(classify)?;
        // Session token first, on a fresh minter, before any other identifier — Metrolist enforces
        // that ordering under a mutex and there is no reason to find out the hard way why.
        let token = bg.mint_token(session_ident).await.map_err(classify)?;
        let it =
            integrity_token_len(&bg.mint_token(CLASS_PROBE).await.map_err(classify)?, CLASS_PROBE);
        let lifetime = u64::from(bg.lifetime());
        if it.is_some_and(|n| n <= ACCEPTED_MAX_IT) {
            tracing::info!(
                attempt,
                it_bytes = it,
                ttl = lifetime,
                "PoToken minter ready (accepted integrity-token class)"
            );
            return Ok((bg, token, lifetime));
        }
        tracing::debug!(
            attempt,
            it_bytes = it,
            "botguard bootstrap in the rejected class, re-rolling"
        );
        last = Some((bg, token, lifetime, it));
    }
    // Keep the last one rather than degrade with nothing. A rejected-class token costs one HEAD
    // (the orchestrator validates every URL and falls through cleanly), and it might work: the
    // 61-62 / 65-66 byte split is one measurement session's, so if YouTube moves the wrapper by a
    // byte then every mint misclassifies and this warn is the only thing that says so.
    let (bg, token, lifetime, it) = last.expect("MAX_BOOTSTRAPS >= 1");
    tracing::warn!(
        it_bytes = it,
        ttl = lifetime,
        "no accepted-class integrity token in {MAX_BOOTSTRAPS} bootstraps, using a rejected-class \
         token (expect 403s; if this keeps happening the class arithmetic has drifted)"
    );
    Ok((bg, token, lifetime))
}

/// Recover the `/GenerateIT` integrity token's size from a minted pot, which is the only way to see
/// which class the runtime landed in. `None` if the pot does not parse or is implausibly short.
///
/// `mint_token` percent-decodes the identifier before binding it, so the length that counts is the
/// decoded one (visitorData routinely arrives with `%3D` padding).
fn integrity_token_len(pot: &str, ident: &str) -> Option<usize> {
    // `mint_token` returns padded base64url; the harness scripts print it unpadded. Take either.
    let bytes =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(pot.trim_end_matches('=')).ok()?;
    let ident_len = urlencoding::decode(ident).map_or(ident.len(), |d| d.len());
    bytes.len().checked_sub(ident_len + POT_OVERHEAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real tokens from the desktop app's own harness, both HEAD-tested against one live WEB_REMIX
    /// stream URL. If this arithmetic drifts, every mint is classified wrong and WEB_REMIX silently
    /// stops resolving again, so pin it to measured bytes.
    #[test]
    fn integrity_token_len_recovers_the_class() {
        let accepted = "MlXqPDpAlDMkCOoCacp97o6hlNiTzyEqbbG19_M6xd4CsWjs_-As_87AhhsJc8P0TBrOBz-NyIZOkBW27jzq9IdgG3iWL8UIP2Z1mH_re9kA4X-E0VNg";
        let rejected = "MlnHbcTsx94ZOMcvkb-5qM8WsZx58c29FKoGcMaRe-7qyQbSAGmxZKrMAr-_WcwQ3Ayn6QcZJUyQwkDM71r6lMts9NLt3mH9kVN6Ni7xo7et0t-naMX8Jore2A";
        assert_eq!(integrity_token_len(accepted, "PtHEr7siapo"), Some(62));
        assert_eq!(integrity_token_len(rejected, "PtHEr7siapo"), Some(66));
        assert!(integrity_token_len(accepted, "PtHEr7siapo").is_some_and(|n| n <= ACCEPTED_MAX_IT));
        assert!(!integrity_token_len(rejected, "PtHEr7siapo").is_some_and(|n| n <= ACCEPTED_MAX_IT));
    }

    #[test]
    fn js_errors_are_transient() {
        assert!(matches!(classify(BgError::Js("boom".into())), Error::Transient(_)));
        assert!(matches!(classify(BgError::InvalidPoToken("nope".into())), Error::Transient(_)));
    }

    #[test]
    fn integrity_token_len_rejects_garbage() {
        assert_eq!(integrity_token_len("not base64!!", "PtHEr7siapo"), None);
        assert_eq!(integrity_token_len("MlU=", "PtHEr7siapo"), None, "shorter than the overhead");
    }

    #[test]
    fn integrity_token_len_uses_the_decoded_identifier() {
        assert_eq!(
            integrity_token_len("MlXqPDpAlDMkCOoCacp97o6hlNiTzyEqbbG19_M6xd4CsWjs_-As_87AhhsJc8P0TBrOBz-NyIZOkBW27jzq9IdgG3iWL8UIP2Z1mH_re9kA4X-E0VNg", "abcdefgh%3D"),
            Some(64),
            "%3D is one byte, not three"
        );
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
///   cargo test -p limusic-core botguard_mints_an_accepted_token -- --ignored --nocapture
#[cfg(test)]
mod live {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    #[ignore = "hits live YouTube"]
    async fn botguard_mints_an_accepted_token() {
        let visitor = std::env::var("LIMUSIC_VISITOR_DATA")
            .expect("set LIMUSIC_VISITOR_DATA to the app's visitor_data setting");
        let video_id = std::env::var("LIMUSIC_VIDEO_ID").unwrap_or_else(|_| "PtHEr7siapo".into());

        let t0 = Instant::now();
        let b = Minter::spawn(crate::http::WEB_UA.to_owned(), visitor.clone())
            .await
            .expect("bootstrap");
        let boot = t0.elapsed();
        let t1 = Instant::now();
        let pot = b.minter.mint(&video_id).await.expect("mint");
        let it = integrity_token_len(&pot, &video_id);
        println!(
            "bootstrap {boot:?} (ttl {}s) | mint {:?} it {it:?}\npot {pot}",
            b.lifetime_secs,
            t1.elapsed()
        );
        assert!(it.is_some_and(|n| n <= ACCEPTED_MAX_IT), "mint fell out of the accepted class");

        // The idle teardown drops the minter, which frees the V8 isolate; the next track start
        // builds a fresh one on the same thread. This is the #133 crash: before the singleton
        // thread, the second isolate died with SIGSEGV / SEGV_PKUERR.
        drop(b);
        let again = Minter::spawn(crate::http::WEB_UA.to_owned(), visitor)
            .await
            .expect("second bootstrap after teardown");
        let pot2 = again.minter.mint(&video_id).await.expect("mint after teardown");
        println!("after teardown: it {:?}", integrity_token_len(&pot2, &video_id));
        assert!(integrity_token_len(&pot2, &video_id).is_some_and(|n| n <= ACCEPTED_MAX_IT));
    }
}
