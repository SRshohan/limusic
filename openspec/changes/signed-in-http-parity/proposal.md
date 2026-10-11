## Why

The two existing HTTP vertical slices (`http-search-vertical-slice`, implemented; `http-browse-vertical-slice`,
planned) deliberately stayed anonymous-only — proving `crates/innertube` and the frontend's
`isTauri` dispatcher work before touching the much harder question of signed-in state. That
question is the only thing left blocking the browser/PWA build from doing most of what the
desktop app does: liked songs, library, history, playlists you own, ratings, lyrics-source
preference, Last.fm, Listen Together, and the real-time push events every one of those features
depends on (now-playing, queue changes, auth changes, …) are all still explicit `unsupported(...)`
stubs in `ui/src/lib/api.web.ts`. This change ports that remaining surface, following the project's
own master conversion plan (`/Users/sohanurrahman/.claude/plans/snug-pondering-kahan.md`) — its
Phase 2 ("HTTP core") plus the non-streaming parts of Phase 4 ("wire into the real frontend").

It deliberately does **not** include actual YouTube audio streaming (that plan's Phase 3): the plan
itself isolates headless cipher/BotGuard execution and the V8-singleton-thread constraint
(documented crash history, issue #133) as its own go/no-go milestone, and bundling it here would
mix a large, novel execution-risk problem into what is otherwise a mechanical port of already-working
logic to a new transport. Pressing play will still do nothing at the end of this change — every
page will show real data, nothing will make sound yet.

## What Changes

- **New crate layout**: fork-and-freeze `src-tauri`'s Tauri-free logic into `crates/core` (a
  library with no `tauri::AppHandle`/`Manager`/`Emitter` dependency), consumed by both the existing
  `crates/server` binary and, unchanged, `src-tauri` itself (which keeps building against its own
  copy until a later change retires it — see design.md). `state.rs`'s ~20 `app.emit(...)` call
  sites become a plain `tokio::sync::broadcast`-backed `Broadcaster`, replacing the Tauri event
  bus with a transport-agnostic one `crates/server` can expose over a WebSocket.
- **Server-side session storage**: `crates/server` gains its own account/session store (same shape
  as `src-tauri/src/db.rs`'s `accounts`/`settings` tables — cookie, `data_sync_id`, `visitor_data`
  per account, keyed by the same `account_key()` SAPISID hash), issuing an `HttpOnly` session
  cookie on login. This is the credential-custody model the user chose over a browser-held cookie.
- **Auth routes**: cookie-paste sign-in (`README.md`'s already-documented alternative to the
  in-app Google webview login — no webview to host in a browser, so this is the only v1 login
  method; real OAuth is explicitly later), sign-out, account identities, account switching.
- **New HTTP routes**, grouped by domain, for every remaining read/write command in
  `ui/src/lib/api.web.ts`'s `unsupported(...)` list that isn't inherently playback: library
  (liked songs, saved albums/artists, uploads, history, play counts), playlist write actions
  (create/edit/delete/add/remove/sort/cover — viewing is already `http-browse-vertical-slice`'s
  job), ratings (rate a song, save a song/album, subscribe to an artist), settings (the server-side
  equivalent of the desktop key/value store — excluding `setAppIcon`/`appIconPath`/`allowFontFile`,
  which have no browser equivalent and stay unsupported permanently), lyrics, Last.fm, blocked
  artists, and diagnostics.
- **Listen Together, wired without any new backend route**: the browser connects directly to the
  user-configured `crates/sync-server` relay over its own `WebSocket`, the same way the desktop
  app's `listentogether/mod.rs` already does — see design.md. Nothing in `crates/core`/
  `crates/server` is involved.
- **One `/ws` endpoint** unifying the remaining ~20 Tauri event names (`now-playing`,
  `auth-changed`, `queue-changed`, …) into a single typed envelope, replacing `onX(...)` subscriptions
  in `api.web.ts`.
- **Frontend wiring**: replace every non-playback `unsupported(...)` stub in `api.web.ts` with a
  real `fetch`/WebSocket implementation. Remove the remaining Tauri-only gating in
  `ui/src/routes/+layout.svelte` that still hides `Sidebar`/`PlayerBar` outside Tauri, now that the
  data those components need is actually reachable — a lighter-weight equivalent replaces whatever
  of that chrome makes sense without playback (e.g. the sidebar's nav, not a player bar with
  nothing to control yet).
- **BREAKING for nothing existing**: `src-tauri` is untouched and keeps working exactly as today;
  this is additive to the browser/PWA surface only.

## Capabilities

### New Capabilities
- `http-auth`: signing in (cookie-paste), signing out, listing and switching Google account
  identities, over HTTP with a server-held session.
- `http-library`: viewing the signed-in user's library — liked songs, saved albums, saved artists,
  uploads, watch history, local play counts — over HTTP.
- `http-playlists-write`: creating, editing, deleting, sorting, and changing the membership/cover
  of a playlist the signed-in user owns, over HTTP.
- `http-ratings`: rating a song, saving a song or album to the library, and subscribing to an
  artist, over HTTP.
- `http-settings`: reading and writing the signed-in user's server-side settings, and clearing
  server-side caches, over HTTP.
- `http-lyrics`: fetching lyrics, choosing an alternate source, adjusting sync offset, and listing
  available providers, over HTTP.
- `http-lastfm`: connecting, checking status, and disconnecting Last.fm scrobbling, over HTTP.
- `http-blocked-artists`: listing, blocking, and unblocking artists, over HTTP.
- `http-listen-together`: hosting and joining a synced-listening room (room lifecycle, approvals,
  host transfer), over a direct browser connection to a user-configured relay — not a capability of
  `crates/server`/`crates/core` itself (see design.md); listed here because it's still part of the
  frontend parity this change delivers.
- `http-realtime-events`: a single real-time channel over which the server pushes the events every
  other capability above depends on for live updates (auth changes, library changes, Last.fm state,
  login outcome).
- `http-diagnostics`: retrieving diagnostics text and a summary, and saving diagnostics to a file
  the browser controls (not a server-side path, since the browser has no filesystem access to one).

### Modified Capabilities
_None._ `search-http-api` and `browse-http-api` (from the two prior changes) are unchanged in
external behavior — this change relocates their implementation into the new `crates/core`/
`crates/server` structure as part of the broader extraction, but neither route's request/response
contract changes, so neither has a requirement-level delta here.

## Impact

- New library crate `crates/core`, forked (trimmed — see design.md) from
  `src-tauri/src/{state,db,commands,lyrics,lastfm,session,romanize,blocked,diagnostics}`, with
  every `tauri::`-specific dependency removed or replaced (`AppHandle`/`Manager`/`Emitter` →
  `Broadcaster`). Not forked: `orchestrator`/`audioproxy`/`videoproxy`/`potoken`/`cipher`
  (streaming, out of scope here), `discord` (nothing in this change routes to it yet), and
  `listentogether` (it's a WS client to the already-standalone `crates/sync-server` relay, not
  server-side logic to fold in — see design.md). `local.rs` and its SQLite-backed local-music
  tables are explicitly **not** forked — the browser's local-music feature already has its own
  separate, IndexedDB-based system from `offline-local-music-pwa`, and a server-side local-library
  mode is an explicit later stretch item in the master plan, not part of this change.
- `crates/server` gains a real domain-routed structure (`routes/*.rs` per domain, `ws.rs`), a
  session/account store, and depends on the new `crates/core` instead of calling `crates/innertube`
  directly.
- `src-tauri` is **unchanged** — it keeps its own copy of the forked logic until a later change
  migrates it onto `crates/core` and retires the duplication.
- `ui/src/lib/api.web.ts` loses most of its remaining `unsupported(...)` stubs; a small, explicit
  subset stays unsupported permanently (desktop-only: app icon, font file, global hotkeys, tray,
  mini player, native video/ambient frame, in-app zoom, self-update — per the master plan's own
  "Explicit scope cuts").
- `ui/src/routes/+layout.svelte` and `ui/src/lib/components/Sidebar.svelte` gain real, working
  behavior outside Tauri instead of being hidden by the `isTauri` branch.
- No change to playback, streaming, or any command in the Non-Goals list below — a later change's
  job.
