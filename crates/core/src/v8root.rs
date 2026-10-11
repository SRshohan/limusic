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

use std::sync::OnceLock;
use std::thread;

use tokio::sync::mpsc;

/// One unit of work for the v8 thread. Each variant is handled by its own module's `handle()`,
/// against that module's own persistent, thread-local-to-the-v8-thread state — this module only
/// owns the thread and the dispatch loop, not the BotGuard/cipher logic itself.
pub(crate) enum Job {
    BotGuard(crate::potoken::botguard::Cmd),
    Cipher(crate::cipher::jshost::Cmd),
}

fn host() -> &'static mpsc::UnboundedSender<Job> {
    static HOST: OnceLock<mpsc::UnboundedSender<Job>> = OnceLock::new();
    HOST.get_or_init(|| {
        let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
        // If the thread cannot start, or tokio cannot be built on it, `rx` drops here and every
        // later `send` fails — the permanent-degradation latch both BotGuard and the cipher
        // already treat as `Error::Fatal`.
        let _ = thread::Builder::new().name("v8root".into()).spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
                return;
            };
            rt.block_on(async move {
                let mut botguard_state = crate::potoken::botguard::State::default();
                let mut cipher_state = crate::cipher::jshost::State::default();
                while let Some(job) = rx.recv().await {
                    match job {
                        Job::BotGuard(cmd) => {
                            crate::potoken::botguard::handle(&mut botguard_state, cmd).await;
                        }
                        Job::Cipher(cmd) => {
                            crate::cipher::jshost::handle(&mut cipher_state, cmd).await;
                        }
                    }
                }
            });
        });
        tx
    })
}

/// Send a job to the v8 thread. The only failure mode is the thread never having started at all
/// (never replaced afterward, so this stays failed for the life of the process once it happens).
pub(crate) fn send(job: Job) -> Result<(), ()> {
    host().send(job).map_err(|_| ())
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
}
