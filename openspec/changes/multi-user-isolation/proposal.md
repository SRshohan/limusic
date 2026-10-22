## Why

`crates/server` (the HTTP backend behind the web/PWA build) supports exactly one signed-in YouTube
account per running process today. `signed-in-http-parity`'s design.md made this an explicit
Non-Goal when the session system was first built, and the code still matches that decision:
`RequireSession` proves a request carries *a* valid session cookie, but every one of those cookies
ultimately resolves against the same single `AppState.it: InnerTube` — one shared cookie jar, one
`Db`, one "active account." A second person signing in today silently replaces the first person's
session; there is no way for two people to be signed into their own accounts on the same running
server at the same time.

For a self-hosted deployment meant to be shared — a household or small friend group pointed at one
VPS instance — this is the actual blocker, not a missing feature on top of a working multi-user
base. This change makes concurrent, independent sign-in real: each person gets their own session,
their own YouTube account state, and their own library/playlists/settings/history, isolated from
everyone else using the same server. It is sized for a handful to a few dozen concurrent users, not
a public multi-tenant SaaS.

## What Changes

- **Per-user account state, not a single shared one.** `AppState`'s account-specific fields
  (`it: InnerTube`, the active-account bookkeeping, and the account-scoped parts of `Db`) stop being
  one value the whole process shares and become one value *per signed-in user*, created on sign-in
  and looked up per request from the session cookie. **BREAKING** for the existing single-account
  assumption: `RequireSession`'s extra `state.app.it.is_logged_in()` check (today's de facto
  "only one account can ever be live" gate) is removed — a valid session is sufficient on its own,
  because it now resolves to an independently-live account rather than a shared global one.
- **Deliberately NOT duplicated per user**: `cipher` (signature-cipher deobfuscation) and `potoken`
  (BotGuard attestation) stay single, shared, process-wide instances. Neither is tied to a login —
  they decode YouTube's public player.js and mint anonymous bot-attestation tokens respectively —
  and duplicating them per user would multiply this process's heaviest resources (an embedded V8
  runtime each) for no isolation benefit.
- **Account-scoped database rows.** Every table that implicitly assumed "the one active account"
  (library, playlists, settings, play history, blocked artists, …) gains real per-account scoping,
  so two different people's data coexists in the same database without either seeing the other's.
- **Sign-in flow unchanged in shape, different in effect.** The existing cookie-paste flow
  (`POST /api/auth/cookie`) still works exactly as a user experiences it; what changes is that a
  second person using it no longer evicts the first person's live session.
- **A documented concurrency ceiling.** The design states a concrete target (low tens of concurrent
  signed-in users) and what actually bounds it (memory per cookie jar, SQLite connection handling),
  rather than leaving "how many users is this actually good for" unanswered.
- **Out of scope, explicitly:** playback/queue state (already fully client-owned per browser tab,
  per `http-audio-streaming`'s design D5 — nothing server-side to isolate there) and Listen Together
  (`crates/sync-server` — a different, already-multi-party-by-design feature for people sharing one
  listening session on purpose; untouched by this change).

## Capabilities

### New Capabilities
- `multi-user-isolation`: the cross-cutting guarantee that concurrent signed-in users' account
  state — session, YouTube login, library, playlists, settings, and watch/play history — stays
  independent and invisible to one another on the same running server, plus the concurrency ceiling
  this design targets and is bounded by.

### Modified Capabilities
- `http-auth` (defined in `signed-in-http-parity`, not yet archived): the session model changes
  from "a cookie proving you signed into *the* account this process happens to hold" to "a cookie
  bound to *your own*, independently-live account state." Sign-in/sign-out/identity-switching
  behavior is observably the same for one user acting alone; it changes when a second person is
  signed in concurrently, which today silently breaks the first person's session and after this
  change must not.

## Impact

- **Affected code**: `crates/core/src/state.rs` (`AppState` restructuring), `crates/core/src/db.rs`
  (schema — account-scoping columns/migration), `crates/server/src/session.rs` (`SessionStore` /
  `RequireSession`), every route handler in `crates/server/src/*.rs` that currently reads
  `state.app` as if it were singular.
- **Not affected**: `crates/core/src/cipher/`, `crates/core/src/potoken/`,
  `crates/core/src/media/{audio_proxy,video_proxy}.rs` (stay shared/global by design — see
  design.md), `ui/src/lib/web-playback.svelte.ts` (client-owned queue, no server-side isolation
  needed), `crates/sync-server` and `ui/src/lib/lt-client.ts` (Listen Together, a separate
  feature), `src-tauri` (desktop keeps its own single-account-per-process model unchanged — this
  change is `crates/server`-only).
- **Deployment**: no change to how the server is deployed (still one Docker container, one Caddy
  site) — the isolation is internal to the process, not a new container-per-user pattern.
