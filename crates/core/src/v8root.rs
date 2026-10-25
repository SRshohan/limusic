//! The one OS thread allowed to touch V8 in this process (openspec/changes/http-audio-streaming,
//! design.md D1).
//!
//! `src-tauri/src/potoken/botguard.rs`'s own header comment states the constraint this exists to
//! satisfy: V8's default platform write-protects its JIT pages with a memory-protection key, and
//! that key is per-thread, inherited only at OS-thread-clone time — so every thread that ever
//! touches V8 must be a descendant of whichever thread first called `v8::Initialize`, or the first
//! thing V8 touches on it faults with `SIGSEGV` (issue #133: a second thread touching V8 crashed
//! the whole app).
//!
//! The master plan's own Phase 3 text proposes a dedicated root thread that spawns both the
//! BotGuard worker thread and a new cipher worker thread as its children. Reading the existing
//! `botguard.rs` shows that doesn't quite match how it actually establishes its root: there is no
//! pre-existing neutral thread today — the lazily-spawned `"botguard"` OS thread *is* the thread
//! that ends up calling `v8::Initialize`, transitively, inside its own closure. Rather than
//! retrofit a neutral thread that spawns that one as a child plus a cipher thread as a sibling
//! (more moving parts, and BotGuard's `host()` would have to stop self-spawning), this module
//! keeps exactly **one** long-lived OS thread, running one `tokio::runtime::Builder::
//! new_current_thread()`, and both BotGuard's and the cipher's `!Send` runtimes live and are
//! driven entirely within that one thread's loop. This is correct by construction: there is only
//! ever one thread touching V8 in the whole process, so "must be a descendant" is trivially
//! satisfied — there is nothing else to be a descendant of.
//!
//! **Self-healing, added after a real incident**: a panic while handling one job (a BotGuard/cipher
//! bug, or a V8-internal assertion tripped by something YouTube's JS shipped that day) used to take
//! the whole thread down with it — Rust's default behavior for an unhandled panic on a spawned
//! thread. Every later job's `send` would then quietly fail forever, which both `botguard.rs` and
//! `jshost.rs` had only ever been able to treat as "permanently degrade" (there was nothing else
//! they could do — this module offered no way back). That is exactly what happened: BotGuard
//! panicked mid-bootstrap, the thread died, and the process silently lost PoToken minting (and the
//! cipher's own realtime path) for the rest of its uptime, surfacing days later as "no client could
//! resolve a playable stream" with no restart in between to explain it.
//!
//! Two layers now guard against that recurring:
//! 1. Each job runs behind its own `catch_unwind` boundary (below) — a panic handling one job ends
//!    that one job, not the thread. The two modules' own state is rebuilt fresh afterward (a V8
//!    isolate that panicked mid-operation cannot be trusted to still be internally consistent), and
//!    the loop keeps serving the next job exactly as before.
//! 2. [`send`] detects a thread that's gone anyway (the channel's receiver dropped — this thread
//!    never started at all, or died from something `catch_unwind` itself can't catch, like a stack
//!    overflow) and transparently respawns a fresh one before retrying, rather than failing forever.

use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, OnceLock};
use std::thread;

use futures_util::FutureExt;
use tokio::sync::mpsc;

/// One unit of work for the v8 thread. Each variant is handled by its own module's `handle()`,
/// against that module's own persistent, thread-local-to-the-v8-thread state — this module only
/// owns the thread and the dispatch loop, not the BotGuard/cipher logic itself.
pub(crate) enum Job {
    BotGuard(crate::potoken::botguard::Cmd),
    Cipher(crate::cipher::jshost::Cmd),
}

/// Start a fresh v8 thread and return the sender that feeds it. Never fails outright: if
/// `thread::Builder::spawn` itself can't create an OS thread (resource exhaustion), the closure
/// (and the receiver it captured) is simply dropped, so the returned sender is already closed —
/// [`send`]'s own retry-after-respawn logic notices that on the very next call.
fn spawn_thread() -> mpsc::UnboundedSender<Job> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
    let _ = thread::Builder::new().name("v8root".into()).spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
            return;
        };
        rt.block_on(async move {
            let mut botguard_state = crate::potoken::botguard::State::default();
            let mut cipher_state = crate::cipher::jshost::State::default();
            while let Some(job) = rx.recv().await {
                let panicked = match job {
                    Job::BotGuard(cmd) => {
                        AssertUnwindSafe(crate::potoken::botguard::handle(&mut botguard_state, cmd))
                            .catch_unwind()
                            .await
                            .is_err()
                    }
                    Job::Cipher(cmd) => {
                        AssertUnwindSafe(crate::cipher::jshost::handle(&mut cipher_state, cmd))
                            .catch_unwind()
                            .await
                            .is_err()
                    }
                };
                if panicked {
                    tracing::error!(
                        "v8 thread: a job panicked; the thread survives, both modules' state was reset"
                    );
                    botguard_state = crate::potoken::botguard::State::default();
                    cipher_state = crate::cipher::jshost::State::default();
                }
            }
        });
    });
    tx
}

static HOST: OnceLock<Mutex<mpsc::UnboundedSender<Job>>> = OnceLock::new();

/// Send a job to the v8 thread, transparently respawning it first if it's gone — see this module's
/// header comment. A respawn means fresh BotGuard/cipher state, which every caller already treats
/// the same as any other transient failure (a cold cache, not a permanent one).
pub(crate) fn send(job: Job) -> Result<(), ()> {
    let mut guard = HOST.get_or_init(|| Mutex::new(spawn_thread())).lock().unwrap();
    match guard.send(job) {
        Ok(()) => Ok(()),
        Err(mpsc::error::SendError(job)) => {
            tracing::warn!("v8 thread was gone; respawning a fresh one");
            *guard = spawn_thread();
            guard.send(job).map_err(|_| ())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    /// The actual property design.md D1 depends on: a BotGuard job and a Cipher job, sent
    /// concurrently, are both served on the very same OS thread — not two different ones racing
    /// for the same V8 process. Each side's `Cmd::Probe` reports `thread::current().id()` from
    /// inside its own `handle()`, run through this module's real dispatch loop (not a mock).
    #[tokio::test]
    async fn botguard_and_cipher_jobs_run_on_the_same_dedicated_thread() {
        let (bg_reply, bg_rx) = oneshot::channel();
        let (ci_reply, ci_rx) = oneshot::channel();
        send(Job::BotGuard(crate::potoken::botguard::Cmd::Probe(bg_reply)))
            .expect("v8 thread accepts jobs");
        send(Job::Cipher(crate::cipher::jshost::Cmd::Probe(ci_reply)))
            .expect("v8 thread accepts jobs");
        let bg_thread = bg_rx.await.expect("botguard probe replies");
        let ci_thread = ci_rx.await.expect("cipher probe replies");
        assert_eq!(bg_thread, ci_thread, "both kinds must run on the one shared v8 thread");
        assert_ne!(bg_thread, std::thread::current().id(), "must be a distinct thread, not inline");
    }

    /// Regression test for the real incident this module's header comment describes: a job that
    /// panics must not take the shared v8 thread down with it. Deliberately drives its own
    /// `spawn_thread()` instance rather than going through `send`/`HOST`: that singleton is shared
    /// by every other test in this binary (`cipher::jshost`'s own tests included), and this test's
    /// whole point is to panic a job and reset that thread's state — doing that against the real
    /// shared singleton would be a genuine race against whatever else is using it concurrently.
    #[tokio::test]
    async fn a_panicking_job_does_not_kill_the_shared_v8_thread() {
        let tx = spawn_thread();

        let (probe_before_reply, probe_before_rx) = oneshot::channel();
        tx.send(Job::Cipher(crate::cipher::jshost::Cmd::Probe(probe_before_reply)))
            .expect("the test thread accepts jobs");
        let thread_before = probe_before_rx.await.expect("probe replies before the panic");

        tx.send(Job::Cipher(crate::cipher::jshost::Cmd::Panic))
            .expect("the panicking job still sends fine");

        let (probe_after_reply, probe_after_rx) = oneshot::channel();
        tx.send(Job::Cipher(crate::cipher::jshost::Cmd::Probe(probe_after_reply)))
            .expect("the test thread still accepts jobs after the panic");
        let thread_after =
            probe_after_rx.await.expect("the thread must still be alive and able to reply after a panic");

        assert_eq!(
            thread_before, thread_after,
            "a panicking job must be caught in place, not kill and respawn the whole thread"
        );
    }
}
