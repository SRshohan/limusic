## Context

See proposal.md for motivation and the master plan
(`/Users/sohanurrahman/.claude/plans/snug-pondering-kahan.md`) for the phase this implements
(Phase 3 + the playback-wiring slice of Phase 4). Current state, verified by reading the actual
code rather than trusting the plan's description:

- `crates/core`/`crates/server` (from `signed-in-http-parity`) have `AppState{it, clients, db,
  events, lastfm, auth}` — no player, no orchestrator, no media state. That change's own
  design.md explicitly deferred `orchestrator.rs`, `audioproxy.rs`, `videoproxy.rs`,
  `cipher/`, `webview.rs` as "only matter once streaming exists."
- Today's `v8` dependency graph (`cargo tree -i v8 -p limusic-app`) shows exactly **one** `v8
  v130.0.7`, reached only through `rustypipe-botguard` → `deno_core v0.331.0`. Adding a second,
  independently-resolved `deno_core` dependency for the cipher is the central risk this design
  has to close off.
- `src-tauri/src/potoken/botguard.rs` already solves the V8-singleton-thread constraint — **for
  itself**. Its own header comment states the constraint precisely: PKRU (the memory-protection
  key V8's JIT write-protection depends on) is per-thread and inherited only at OS-thread-clone
  time, so every thread that ever touches V8 must be a descendant of whichever thread first called
  `v8::Initialize` — getting this wrong is a `SIGSEGV`, not a degraded mode (issue #133: a second,
  later thread touching V8 crashed the whole app). The fix already in place: a single OS thread,
  named `"botguard"`, spawned exactly once behind a `OnceLock<mpsc::UnboundedSender<Cmd>>`
  (`host()`), running one `tokio::runtime::Builder::new_current_thread()` for the life of the
  process. `v8::Initialize` happens **inside that thread's own closure** (transitively, the first
  time `rustypipe_botguard` constructs a `Botguard` there) — so the "botguard" thread itself, not
  whoever first calls `host()`, is the actual root. This matters for Decision D1 below: the master
  plan's `v8root.rs` description doesn't quite match this.
- Cipher's current JS execution (`src-tauri/src/webview.rs`'s `Bridge`) — its harness document
  (`HARNESS_HTML`) is a bare globals scaffold (`window._yt_player`, error hooks, a `__slots` bag),
  and grepping `cipher/*.rs` for `document.`/`window.location`/`fetch(` finds nothing but a
  security test asserting a malicious payload is rejected. That check covers only our own
  orchestration code, though, not the *content* of `player.js` itself — a multi-megabyte,
  externally-sourced bundle. Live-testing (task 3.3) showed its top-level module code does
  reference `document`/`XMLHttpRequest`/etc. during its own startup, unrelated to the sig/n
  functions actually needed; see D3's own correction note for the fix (a shallow inert shim, still
  far short of BotGuard's full `deno_web` extension).
- `src-tauri/src/audioproxy.rs`'s actual mechanism (read directly, not assumed): `register(url,
  headers) -> Option<String>` stores the real upstream URL + headers in an in-memory map keyed by
  a random id, and returns a loopback URL embedding that id plus a process-startup random security
  token; the local hyper server re-issues bounded `CHUNK = 4 MiB` range requests upstream to
  defeat googlevideo's throttling of open-ended ranges (measured: 32 KB/s open-ended vs. 9.4 MB/s
  bounded, per the file's own comment), and serves them back to whatever opened the loopback URL
  (mpv). The random-id+token indirection exists only to stop an arbitrary **local process** from
  guessing a loopback port — not a relevant threat once this lives behind a real HTTP server with
  its own origin and CORS policy (Decision D4).

## Goals / Non-Goals

**Goals:**
- A browser tab plays a real YouTube track, and watches a real music video, anonymously or signed
  in — same fallback-client chain, same cipher/PoToken pipeline the desktop app already uses.
- Exactly one `v8` build in the process after this change (`cargo tree -i v8` still shows one).
- `player.svelte.ts`'s queue/transport becomes real on the web build, reusing
  `ui/src/lib/engine/{audio-engine.ts,crossfade.ts,mediasession.ts}` unmodified.
- Listen Together hosting works from a browser tab (guest/room-management already does; this
  gives a host something to broadcast).

**Non-Goals:**
- Pitch/tempo — dropped entirely, no Web Audio equivalent, not even a stretch item (matches
  `offline-local-music-pwa`'s own precedent for the same gap).
- `src-tauri` is not touched. Cipher/BotGuard/proxies/orchestrator are forked copies into
  `crates/core` — same frozen-reference stance as every prior change in this series.
- No change to the `/ws` event envelope or the already-shipped signed-in command surface — purely
  additive.
- Server-side local-music mode, true multi-user isolation, real OAuth login — all still out of
  scope, unchanged from `signed-in-http-parity`'s own stance.

## Decisions

### D1: One shared "v8 thread," not two independently-rooted ones
The master plan's Phase 3 text proposes a `v8root.rs`: "a `OnceLock`-guarded root thread spawned
once at process start, whose only job is to spawn both the BotGuard worker thread and the new
cipher worker thread *from within itself*." Reading `potoken/botguard.rs` shows this doesn't quite
match how the existing code actually establishes its root: there is no pre-existing neutral "root"
thread today. The lazily-spawned `"botguard"` OS thread **is itself** the thread that ends up
calling `v8::Initialize` (transitively, inside its own closure, the first time it bootstraps).
Retrofitting a separate neutral thread that spawns this one as a child, plus a new cipher thread
as a sibling, would mean restructuring `botguard.rs`'s `host()` to stop self-spawning — more
moving parts for no real benefit.

**Decision**: keep exactly one long-lived OS thread (ported from `botguard.rs`'s existing pattern,
generalized into `crates/core/src/v8root.rs`), running one
`tokio::runtime::Builder::new_current_thread()`, hosting **both** the existing `Botguard`
command loop and a new cipher `deno_core::JsRuntime` command loop, multiplexed over one `Cmd`
enum (`Cmd::BotGuard(...)` / `Cmd::Cipher(...)`) into one `rx.recv()` loop. Both runtimes are
`!Send` and already must be constructed/used thread-locally without crossing an `.await` boundary
that could resume elsewhere — exactly the constraint the existing code already respects for
BotGuard alone. This guarantees correctness **by construction**: there is only ever one thread
touching V8 in the whole process, so "must be a descendant" is trivially satisfied — there is
nothing else to be a descendant of.

**Alternative considered**: spawn a true child thread for the cipher from within the botguard
thread's own closure (matching the plan's literal wording). Rejected: a second channel hop, and a
second `tokio::runtime` would need to nest inside the first's `block_on` or be driven by yet
another dedicated thread — more moving parts than the single shared loop above, for no benefit.

### D2: `deno_core` version pinning
`deno_core` becomes a **direct** dependency of `crates/core`, pinned to the exact version already
resolved transitively (`=0.331.0`, confirmed above). A loose version requirement risks cargo
resolving two different `deno_core`/`v8` builds in one process — not a degraded mode, a hard
crash. `cargo tree -i v8` is re-checked as an explicit task-level verification step after adding
the dependency, not just noted here once.

### D3: JsBridge keeps the same method shape, minus `AppHandle`
Port `webview.rs`'s `Bridge::{create, eval, eval_json, call_async, destroy}` method shape into a
new `crates/core/src/cipher/jshost.rs`, backed by `deno_core::JsRuntime::execute_script` instead
of `WebviewWindow::eval_with_callback`. `eval_json`'s synchronous-value round trip is actually
simpler with `deno_core` (`execute_script` returns the value directly via `serde_v8`, no callback
dance), but `call_async`'s poll-the-global-slot mechanism is kept **unchanged in v1** even though
`deno_core` can natively await a JS promise — matching the master plan's explicit call to prove
behavioral equivalence first and treat native-promise simplification as later cleanup.
`cipher/mod.rs`'s calling code changes only by deleting the `AppHandle` param at its handful of
call sites; the discovery JS, sig/n invocation, and self-heal retry logic are otherwise ported
file-for-file.

**Correction found during task 3.3's live verification** (not assumed — this is where it was
actually caught): this section originally also claimed no real DOM was needed at all, reasoning
from the fact that `cipher/*.rs` (the Rust orchestration code) never touches `document`/`fetch`/
`window.location`. That check only covered our own code, not the *content* of `player.js` — a
multi-megabyte, externally-sourced bundle. Live-testing against a real, current player.js showed
its top-level module code (unrelated to the sig/n functions actually called) references
`document`, `XMLHttpRequest`, and several other browser globals during its own initialization,
well before execution ever reaches the functions this pipeline needs — a bare `JsRuntime` throws a
`ReferenceError` partway through the whole file, so `_cipherSigFunc`/`_nTransformFunc` never get
assigned at all. Fixed with a deliberately shallow, inert shim layer in the harness (`document`,
`navigator`, `location`, `performance`, `XMLHttpRequest`, `fetch`, timers, storage, observers, a
few more) — just enough for player.js's startup code to run to completion without crashing; none
of it needs to behave correctly, since the cipher never calls into it. This is still far short of
`deno_web`'s full extension (confirmed live: the shim was sufficient on the first real player.js
tested), so the core claim — no snapshot-probing, no `deno_web` dependency for the cipher — holds;
only the "zero shim at all" part of the original claim was wrong.

### D4: Stream resolution cache, keyed by videoId
`GET /api/playback/resolve?videoId=` calls the ported `orchestrator::resolve()` and returns only
the client-safe subset of `PlaybackData` (title/artists/duration/thumbnail, and `stream_url`
rewritten to `/media/stream/{videoId}`) — never the real googlevideo URL or its headers
(`PlaybackData.headers` is already `#[serde(skip)]`). The real URL + headers + expiry are kept
server-side in an in-memory map keyed by **videoId** (this change's equivalent of
`audioproxy.rs`'s `Registry`, simplified: the random-id+startup-token indirection is dropped,
since it solves a different threat — an arbitrary *local* process guessing a loopback port — not
relevant behind a real HTTP origin with its own CORS policy). The `/media/stream/:videoId` and
`/media/video/:videoId` handlers look up this map; a miss or an expired entry
(`expires_in_seconds`) triggers an internal re-resolve before proxying, so a long-paused-then-
resumed playback doesn't fail on a stale upstream URL.

### D5: Client-owned playback and queue state for the web build
On desktop, `player.svelte.ts` is pure remote control: `AppState`'s orchestrator/player own the
queue and now-playing state and push it to the frontend via Tauri events (`onNowPlaying`,
`onQueueChanged`, `onPosition`, …), all still wired in `initApp()`. There is no equivalent
server-held state in `crates/server` — the browser *is* the player now, which is the whole point
of reusing `ui/src/lib/engine`. This change gives the web build its own client-owned queue/
now-playing store, modeled directly on the already-shipped `ui/src/lib/local/store.svelte.ts`
(the offline/local-music build's own client-owned queue), feeding the same `playback`/`np`
reactive stores the rest of the UI (`PlayerBar`, `QueuePanel`, `NowPlaying`, …) already reads —
those components don't need to know or care whether a track came from YouTube or a local file.

**Alternative considered**: server-held queue/now-playing state in `crates/server`, closer to the
desktop model and reusing more of `initWebApp`'s existing event-subscription shape. Rejected: it
would need a per-session server-side player object and session-scoped WS broadcast of position/
now-playing — chatty, latent, and pointless for state the browser already has locally. It also
contradicts the master plan's own Phase 4 framing ("`player.svelte.ts` gains real
playback-engine responsibility it never had on desktop").

### D6: Listen Together host broadcast
`lt-client.ts` (`signed-in-http-parity` task 12.10) already has a working `send()` to the relay
and a `role` field; it just never had a `Playback` message to send. Expose a small
`broadcastPlayback(p)` from `lt-client.ts`, and have the new client-owned queue store (D5) call it
on every play/pause/seek/track-change when `lt.role === 'host'` — mirroring the desktop's
`LtSession::broadcast_playback`. No protocol change; `crates/sync-server` is untouched.

## Risks / Trade-offs

- [Risk] Two V8 consumers in one process is exactly the class of bug (issue #133) that already
  bit this codebase once, for one consumer. → Mitigation: D1's single-thread design makes a second
  V8-touching thread structurally impossible rather than relying on discipline; `cargo tree -i v8`
  is a named task-level verification step, not just a design note.
- [Risk] The exact point inside the third-party `rustypipe-botguard` crate where `v8::Initialize`
  actually fires isn't something this plan controls or can fully verify by reading only this
  codebase. → Mitigation: D1 makes this moot — wherever inside `rustypipe-botguard` it happens, it
  happens on the one thread already dedicated to it, and the cipher runtime lives on that same
  thread, so there is no "other thread" for the question to matter to.
- [Risk] googlevideo's throttling/anti-abuse heuristics are unversioned and can change (already
  true today — `cipher/config.rs`'s signature-timestamp/class-probe logic exists because of this
  exact kind of drift). → Mitigation: no code-level fix possible; same operational reality the
  desktop app already lives with (self-heal retry, `teardown_if_idle`), ported unchanged.
- [Risk] Browser autoplay policy (`AudioContext` suspended until a user gesture) behaves
  differently than mpv. → Mitigation: `ui/src/lib/engine/audio-engine.ts` already calls
  `AudioContext.resume()` in `loadTrack`/`play()` (confirmed by reading it) — this change verifies
  that holds up against real server-streamed audio, it doesn't need to solve it again.
- [Trade-off] Dropping the random-id+token indirection from the stream-proxy cache (D4) trades a
  small amount of URL-guessing obscurity for simplicity — acceptable because the desktop
  indirection solves a different threat (an arbitrary local process on the loopback interface),
  not one that applies behind a real HTTP origin with its own CORS policy.

## Migration Plan

Additive only — no existing route, spec, or desktop behavior changes. Land in the order the
decisions above imply: `v8root.rs` + ported BotGuard (relocated, not behaviorally changed) →
cipher `jshost.rs` + ported cipher/orchestrator → media-streaming HTTP routes (independently
verifiable via the master plan's own throwaway `<audio>` test page, before the frontend is touched
at all — this is the go/no-go gate) → frontend wiring. No rollback concern beyond a normal `git
revert` — nothing is migrated in place, no data model changes, `src-tauri` is untouched throughout.
