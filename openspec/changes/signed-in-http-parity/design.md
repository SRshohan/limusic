## Context

See proposal.md for motivation and the master plan (`/Users/sohanurrahman/.claude/plans/snug-pondering-kahan.md`)
for the phase this implements (Phase 2 + the non-streaming parts of Phase 4). Current state:
`crates/server` is a single-file axum binary with one `AppState { it, clients }` and no session
concept (`http-search-vertical-slice`/`http-browse-vertical-slice`). `src-tauri/src/commands.rs`
has **no signed-in enforcement of its own** — `metadata_client(&state)` (the only client-selection
helper, `commands.rs:738`) doesn't check whether anyone is signed in; it works purely because the
desktop app is a trusted single-user process where the frontend itself simply doesn't call
signed-in commands while signed out. That assumption does not hold for an HTTP server, which is
the main new problem this change solves that the prior two slices didn't have to.

Account storage today (`src-tauri/src/db.rs`) is a SQLite `accounts` table keyed by
`account_key()` — an MD5 hash of the `SAPISID`/`__Secure-3PAPISID` cookie value (`db.rs:26`) — plus
a `settings` table holding the *active* account's cookie/`data_sync_id`/`visitor_data` as a
projection. `upsert_account`/`restore_account` (`db.rs:499`, `:592`) move between them on
sign-in/switch/sign-out.

## Goals / Non-Goals

**Goals:**
- Every capability in specs/ gets a real route, enforced so a caller with no established session
  cannot reach a signed-in-only one.
- `crates/core` exists as a real, Tauri-free library crate, forked from the listed `src-tauri`
  modules, that `crates/server` depends on.
- Session storage server-side, matching the user's explicit choice.

**Non-Goals:**
- **`src-tauri` does not adopt `crates/core`.** It keeps its own copy of the forked logic,
  unchanged, for this entire change. Migrating the desktop app onto the new shared crate and
  retiring the duplication is a later, separate change (the master plan's own "frozen... then
  archived" sequencing) — doing both at once would mix "build new server functionality" and
  "refactor a shipping desktop app's internals" into one change, which is exactly the kind of
  bundling CONTRIBUTING.md's "one change per PR" rule exists to prevent. This change accepts
  temporary duplication between `src-tauri` and `crates/core` as the price of that isolation.
- **True multi-user isolation is not in scope.** Matching the master plan's own Phase 2 text ("v1
  is single-active-account per server instance"), this server supports one signed-in account at a
  time per running process — same trust model the desktop app already has, just reachable over
  HTTP instead of IPC. Running multiple independent signed-in users against one process is Phase
  6 stretch scope.
- **No OAuth popup flow, no new login UI beyond cookie-paste.** README.md already documents
  cookie-paste as a supported alternative to the in-app Google webview login; this change reuses
  it verbatim as the only browser login method.
- **No actual audio.** Repeated from proposal.md because it shapes several decisions below: no
  route in this change streams, resolves a stream URL, or touches `orchestrator.rs`'s resolution
  pipeline, `audioproxy.rs`, or `videoproxy.rs`.
- **No server-side local-music mode.** `local.rs`/`local_tracks` stay un-forked; the browser's
  own IndexedDB-based local library (`offline-local-music-pwa`) is unaffected and unrelated.

## Decisions

### `crates/core`: fork, don't share, for this change
**Decision**: copy `state.rs`, `db.rs`, `commands.rs` (split by domain module), `lyrics.rs`,
`lastfm.rs`, `romanize.rs`, `blocked.rs` into a new `crates/core` library crate, plus `http.rs`
(found during implementation — a small, already Tauri-free shared `reqwest` client both
`lastfm.rs` and `lyrics.rs` depend on), with every `tauri::`-specific type (`AppHandle`, `Manager`,
`Emitter`, `#[tauri::command]`) removed. `orchestrator.rs`, `audioproxy.rs`, `videoproxy.rs`,
`discord.rs`, `listentogether/`, `session.rs` are **not** forked in this change — the first three
only matter once streaming exists (Phase 3); `discord.rs` has no route or task anywhere in this
change (checked: not in specs/, not in tasks.md) to justify forking it ahead of actually needing
it, despite the master plan's eventual intent to keep Discord RPC around; `listentogether/` turns
out to be a WS client to an already-standalone relay process, not server-side logic to fold in —
see the dedicated Listen Together decision below; `session.rs` (found during implementation) is
entirely the hidden-webview Google-login UI — irrelevant to cookie-paste, which calls
`AppState::sign_in(cookie: String, ...)` directly with no dependency on anything in that file.
`potoken/`/`cipher/` are **not** forked — streaming, same as `orchestrator.rs`. `local.rs` is
**not** forked (already stated in proposal.md's Impact).
`diagnostics.rs` is **not** forked either (found during implementation): it's built entirely around
desktop concepts that don't transfer — reading `limusic.log` from `AppHandle::path().app_data_dir()`,
detecting AppImage/deb/flatpak install kind. Task 11.1 writes a small, fresh implementation instead
(server version/OS/arch, relevant env vars, no log-file or install-kind concept) rather than forcing
a fork of code that is, in substance, a different feature for a server process.
**Why**: these modules are the ones with real, non-trivial business logic (session/account
management, library/playlist/rating composition, lyrics-provider fallback, Last.fm, Listen
Together's room state machine) that this change needs working correctly over HTTP; the rest either
doesn't apply yet (streaming), has no browser equivalent, or (Discord) simply isn't wired to
anything in this change yet.
**Corrected during implementation**: an earlier draft of this decision listed `orchestrator.rs` and
`discord.rs` as forked. Both were wrong for the same reason — scope not actually backed by a task
or capability in this change. `orchestrator.rs` directly imports
`crate::cipher::CipherDeobfuscator`/`crate::potoken::PoTokenGenerator`, which this same decision
already excludes — forking it would have dragged cipher/PoToken/`deno_core`/V8 into `crates/core`
for a change whose own Non-Goals say no route here touches stream resolution at all. `discord.rs`
compiles fine standalone, but nothing here calls it.
**`state.rs`'s `AppState` is not a clean fork target — it is forked trimmed, not verbatim.**
`AppState` (5,216 lines in `src-tauri`) is a single struct mixing playback/queue/mpv bookkeeping
with the account/library/settings logic this change actually needs: beyond `orchestrator`, it also
holds `player: Player` (the mpv engine itself), `queue: Mutex<QueueState>`, `media: Option<MediaHandle>`,
and roughly eight more fields that are pure playback/queue/OS-media-control bookkeeping
(`generation`, `pending_seek`, `history_pinged`, `is_playing`, `latest_position`,
`last_pos_persist`, `last_media_push`, `last_media_state`, `video_urls`,
`last_queue_fingerprint`/its persistence counterpart) — confirmed none reachable from any
capability this change specs. `crates/core`'s forked `AppState` drops all of them, and every
method that exists only to serve them, down to `it`/`clients`/`db`/`lastfm`
(`discord` and `lt: Arc<LtSession>` both dropped per above — see the dedicated Listen Together
decision for why `lt` isn't needed either) plus whatever account/session/library/playlist/rating/
settings methods remain. `rate_epoch: AtomicU64` is dropped too (found during implementation, not
in the original plan): it exists solely so `refresh_rating` (also dropped — it reads `self.queue`
and `self.app.emit`s a UI-optimistic-update) can detect a stale in-flight refresh. The real `rate`
command (`commands.rs:1072`) never reads `rate_epoch` for its own logic — it only bumps it as a
side effect *for* `refresh_rating`; with `refresh_rating` gone, the bump has nothing left to guard,
and an HTTP request/response model doesn't have this problem for its own commands to begin with — a
`PUT /api/songs/:id/rating` handler can call `state.it.rate(...)` and return the result directly,
no epoch needed. `cache_dir` is dropped too — its one use (`clear_caches` wiping mpv's on-disk audio
cache) doesn't apply server-side; task 6.2 clears only the DB-backed caches
(`stream_url_cache`/`lyrics_cache` tables), not a filesystem path. Several commands also call
`crate::local::is_local_song`/`ALBUM_PREFIX`/`ARTIST_PREFIX`/`UNKNOWN_ARTIST` (`state.rs`,
`lyrics.rs`, `lastfm.rs`) — treated as always "not a local track" in the fork (a request to this
server is YouTube-only by construction; the browser's separate IndexedDB local library never
reaches it), same pattern `http-browse-vertical-slice` already used for `local.rs`'s id prefixes.
This is real surgical editing with judgment calls at each cut, not the mechanical rename task
group 1's own heading implies — tracked explicitly here so the discrepancy isn't silently absorbed.
`src-tauri`'s own `state.rs` is untouched; every trim above exists only in the `crates/core` fork.
**Alternative considered**: fork `orchestrator.rs` anyway behind throwaway stub types for
`CipherDeobfuscator`/`PoTokenGenerator`. Rejected — real code and a compile dependency for
infrastructure this change will never call.
**Alternative considered**: don't fork `state.rs` as a struct at all — build a small, purpose-built
state struct from scratch and port each needed command's logic into it individually. Rejected (the
user's call) — more total work, for a cleanliness benefit the later `src-tauri` migration change
already needs to revisit anyway once the duplication gets retired.
**Alternative considered**: have `crates/server` depend on `src-tauri` directly and strip Tauri
types at the call site. Rejected — `src-tauri` is a binary crate, not structured to be a library
dependency, and keeping it a frozen reference during this change (see Non-Goals) means it can't be
edited to become one without contradicting that same boundary.

### Event bus: `Broadcaster` replacing `AppHandle::emit`
**Decision**: `crates/core` exposes a `Broadcaster` wrapping
`tokio::sync::broadcast::Sender<(String, serde_json::Value)>` with `emit(name, payload)`/
`subscribe() -> Receiver`. Every one of `state.rs`'s ~20 `self.app.emit(name, payload)` call sites
(confirmed count: 20 in `state.rs`, 37 across the crate) becomes `self.events.emit(name, payload)`
— a mechanical rename, not a behavior change, per the master plan.
**Why**: this is the master plan's own designed replacement, chosen specifically because it's a
one-token substitution at every call site rather than a redesign — keeping this change's actual
diff against the forked logic as close to a pure rename as possible, which matters given how much
else this change already touches.

### Auth: opaque server-issued session id, not the YouTube cookie itself
**Decision**: on a successful cookie-paste login, `crates/server` validates the submitted YouTube
cookie (same validation `sign_in_inner`/`state.rs:631` already does), stores the account row (same
shape as today's `accounts` table), and sets its **own**, separately-generated, random session
identifier as an `HttpOnly` cookie on the response. The YouTube cookie itself is never echoed back
to the browser or used as the browser-facing session token.
**Why**: the YouTube cookie is a high-value, long-lived credential; minimizing how often it
transits back to a client (even `HttpOnly`) reduces exposure if a proxy, browser extension, or log
somewhere along the way captures response headers. An opaque server-generated id ties the browser
session strictly to this server's own session store, which is also what lets sign-out invalidate
it without needing to know anything about YouTube's cookie format.
**Alternative considered**: use the YouTube cookie directly as the session token (skip the extra
indirection). Rejected for the exposure reason above, and it would also couple the browser-facing
cookie's shape to YouTube's own, which the existing `account_key()` migration-handling code
(`db.rs:332` and surrounding) shows has already changed format once.

### Enforcement: a `RequireSession` extractor in `crates/server`, not in `crates/core`
**Decision**: add an `axum` extractor (or equivalent middleware) in `crates/server` that resolves
the caller's session cookie to a stored account before a handler for any signed-in-only capability
runs, rejecting with a clear 401-equivalent client error when there is none. This check does
**not** move into `crates/core` itself.
**Why**: `crates/core`'s forked logic has never enforced this (see Context) — the desktop app's
own trust model makes it unnecessary there, and retrofitting an enforcement concept into code
that's meant to stay a close, low-diff fork of `src-tauri` logic would work against the "fork,
don't share" decision above. The check belongs at the transport boundary (`crates/server`), which
is also the only place that knows what an HTTP session even is.
**Alternative considered**: check per-handler, inline, instead of a shared extractor/middleware.
Rejected — eleven-plus signed-in-only routes repeating the same check inline is exactly the kind
of repetition a shared extractor exists to avoid, and a missed inline check is a real security bug
(a route silently serving someone else's library), not just a style nit.

### `/ws`: one endpoint, envelope matches event name 1:1
**Decision**: `GET /ws` upgrades to a WebSocket, authenticated by the same session cookie as HTTP
requests, sending `{"type": "<event-name>", "data": <payload>}` for everything the connected
session's `Broadcaster` subscription yields. Single-account-per-instance (Non-Goals) means there is
only ever one signed-in account's events to push, so no cross-session filtering logic is needed
here — an unauthenticated connection simply gets nothing signed-in-only.
**Why**: matches the master plan's own stated shape exactly, and keeps the frontend's event
dispatch table (already built against named Tauri events) a rename, not a rewrite.

### Listen Together: the browser connects directly to the sync-server relay — not through `/ws`
**Decision**: Listen Together needs **no new Rust code** in `crates/core`/`crates/server` at all.
`ui/src/lib/ws.ts` (or a dedicated `lt.ts`) opens its own, separate native `WebSocket` straight to
whatever relay URL the user configures (the existing `ltSetServerUrl` command's job), speaking
`listen_protocol`'s `ClientMessage`/`ServerMessage` reimplemented as plain TypeScript types —
exactly the JSON-over-`tokio-tungstenite` protocol the desktop app's `listentogether/mod.rs` and
`crates/sync-server` already speak to each other.
**Corrected during implementation**: the original task 10 ("fork the `listentogether/` room state
machine into `/ws` message handling") was based on a wrong read of where that logic lives.
`src-tauri/src/listentogether/mod.rs` is a WebSocket **client** to a separate, already-standalone
relay process (`crates/sync-server`, 1,151 lines — host-authoritative room/participant/grace-period
logic, zero Tauri dependency) — it is not server-side room logic to fold anywhere. The actual rooms
already live in a process this change never needed to touch. `crates/core`'s fork therefore drops
`listentogether/` entirely, and `state.rs`'s `lt: Arc<LtSession>` field plus its
`use crate::listentogether::{LtSession, SyncCommand}` import are trimmed along with the other
playback-adjacent fields in the "fork, don't share" decision above.
**Why**: the desktop app already runs Listen Together exactly this way — connect to a relay URL
you (or whoever's hosting it) configured, same trust model and same wire protocol, no backend
indirection. Routing it through `crates/server`'s `/ws` instead would add a relay hop for no
benefit and require reimplementing `crates/sync-server`'s already-working room logic a second time.
**Alternative considered**: have `crates/server` itself proxy/absorb the sync-server role. Rejected
— `crates/sync-server` already does this, standalone, documented in README.md as its own
`cargo run -p sync-server` process; duplicating it here serves nothing this change needs.

### Diagnostics "save" becomes a browser download, not a server-side file write
**Decision**: `GET /api/diagnostics/save` (or equivalent) returns the diagnostics content as a
downloadable response (`Content-Disposition: attachment`); there is no server-side path parameter
the way the Tauri command's `path: String` argument implies.
**Why**: a browser has no filesystem path to hand the server to write to — the browser's own
save/download flow is the only mechanism that makes sense here, and it's a strictly simpler
contract than inventing a fake path argument the server would have to ignore.

## Risks / Trade-offs

- **[Risk]** Temporary duplication between `src-tauri` and `crates/core` means a bug fix or
  behavior change made in one during this change's development window could silently not reach
  the other. → **Mitigation**: this change does not plan to touch `src-tauri` at all once the fork
  is taken (Non-Goals), so the window is "fork point to this change's completion," not ongoing;
  the later migration change closes the gap permanently by deleting the duplication, not by
  keeping both in sync indefinitely.
- **[Risk]** This is the largest change to this codebase's structure so far — a new crate, ~15
  forked modules, a new auth/session model, and ten-plus new capabilities in one change. The
  blast radius of getting the `crates/core` extraction wrong is high (it's the foundation every
  task after section 1 builds on). → **Mitigation**: tasks.md's section 1 is a pure mechanical
  fork-and-compile step with no new behavior, verified against the existing (already-passing)
  `cargo test --all` before any new route is written — isolating "did the fork preserve behavior"
  from "does the new HTTP layer work" as two separately-verifiable questions.
- **[Risk]** `account_key()`'s own test suite (`db.rs:1594`, "Rows filed under a key the current
  `account_key` no longer computes") shows this hashing scheme has already changed once with a
  migration needed. Forking it into `crates/core` means a future change to it has to be ported to
  two places (until the later migration), or the fork silently drifts. → **Mitigation**: no change
  to `account_key()`'s behavior is planned here; flagging this so whoever picks up the later
  `src-tauri` migration knows to check for drift specifically in this function.
- **[Risk]** Server-side credential storage is a materially different risk than the desktop app's
  single-user local SQLite file — this server process now holds a live YouTube session cookie
  reachable over a network, not just a local disk only the device's user can read.
  → **Mitigation**: this is the exact trade-off the user explicitly chose (server-held session
  over browser-held) after weighing it earlier; no further mitigation beyond the opaque-session-id
  decision above is in scope here — operational hardening (TLS termination, at-rest encryption of
  the account store) is this slice's deployment story the same way `http-search-vertical-slice`'s
  design.md deferred hosting/TLS concerns, not a new concern this change introduces.

### Corrected during implementation: `http-blocked-artists` is anonymous, not signed-in
**Found live-testing task 9.1**: the proposal/spec originally framed blocked artists as a
signed-in feature ("a signed-in browser client view, add to, and remove from *their* blocked-
artists list"). Checking the original Tauri commands turned up no `require_login`/`is_logged_in`
guard on any of `get_blocked_artists`/`block_artist`/`unblock_artist`, and `blocked.rs`'s own
`list`/`block`/`unblock` take only `&Db` — no account parameter exists for this feature to be
scoped by. It's a local blocklist, same trust model as a browser's own bookmark list, not account
state. `specs/http-blocked-artists/spec.md` is corrected to describe it as anonymous; the route
handlers in `crates/server/src/blocked.rs` drop the `RequireSession` extractor they were given.
