## Context

See proposal.md for motivation. Current state `crates/server/src/main.rs` builds on:

```rust
struct AppState { it: InnerTube, clients: Clients }
```

— one `Arc<AppState>`, one `InnerTube::new(Session::default(), None)` (anonymous), one
`Clients::bundled()`, one `CorsLayer` scoped to `http://localhost:5183`, one route
(`GET /api/search`). Every command this change ports already exists in `crates/innertube` and is
already called from `src-tauri/src/commands.rs` through the identical
`metadata_client(&state)` + `state.it.<method>(...)` shape the existing `/api/search` route uses —
see `endpoints.rs`: `search_videos`, `search_all`, `search_suggestions`, `search_cards`, `home`,
`home_continuation`, `moods`, `playlist`, `album`, `artist`, `browse_grid`,
`playlist_continuation`. None of them take a session/cookie argument beyond the same `Session`
`InnerTube` already carries, so no new state is needed.

The frontend side is further along than `http-search-vertical-slice`'s own design.md assumed: that
slice's "inline `isTauri` branch inside `search()`" decision was superseded (by a later, unrelated
change) with a cleaner module-swap dispatcher — `ui/src/lib/api.ts` picks `api.tauri.ts` or
`api.web.ts` once, at module load, based on `isTauri`. `api.web.ts` already has a typed,
individually-named `unsupported(...)` stub for every command this change ports — no new frontend
plumbing, just replacing specific stubs with real `fetch` calls, one function at a time.

## Goals / Non-Goals

**Goals:**
- Every anonymous, read-only browse/search command listed in proposal.md gets a real HTTP route
  and a real `api.web.ts` implementation, in the same shape the existing `search` route/impl pair
  already established.
- `/search` stops crashing outside Tauri, and the root redirect to `/offline` is removed once it
  doesn't.

**Non-Goals:**
- Playback of any kind (`play`, `playIndex`, `playPlaylist`, `startRadio`, queue state, video/audio
  streaming) — a real media-serving story is a separate, later change.
- Anything signed-in (library, history, owned playlists, write actions, accounts) — blocked on the
  credential-custody decision proposal.md defers.
- Changing `crates/innertube` itself, or any `src-tauri`/Tauri command — every method this change
  calls already exists and is reused unmodified, exactly like the prior slice.
- A general-purpose pagination/continuation abstraction — each route takes whatever continuation
  shape its underlying `innertube` method already returns, nothing is unified across routes.

## Decisions

### Split `crates/server/src/main.rs` into modules
**Decision**: move route handlers into `crates/server/src/search.rs` and
`crates/server/src/browse.rs` (one `axum::Router` builder function each), with `main.rs` left to
wire `AppState`, CORS, and `Router::merge` the two. `AppState`/`ApiError` stay shared (a small
`state.rs` or left in `main.rs` — implementer's call, not worth a separate decision).
**Why**: the existing single-route `main.rs` is already at a comfortable size for one handler; this
change adds eleven more. A flat file would bury the routing table the next change (playback, then
auth) will also need to extend.
**Alternative considered**: keep everything in `main.rs`. Rejected — purely a readability call, no
behavior difference, but the file would roughly 5x in size.

### Route shape: one path per command, mirroring its Tauri command name
**Decision**:
- `GET /api/search/all?q=`, `GET /api/search/suggestions?q=`, `GET /api/search/videos?q=`
- `GET /api/search/cards?q=&category=albums|artists|playlists`
- `GET /api/home?params=` (optional), `GET /api/home/more?token=`
- `GET /api/album/:id`, `GET /api/artist/:id`
- `GET /api/playlist/:id?sort=&desc=` (both optional), `GET /api/playlist/more?token=`
- `GET /api/moods`, `POST /api/moods/art` with a JSON body `string[]` (the one write-shaped
  request in this batch — `getMoodArt` takes a list of tile params, awkward to cram into a query
  string, and the Rust command already takes `Vec<String>`)
- `GET /api/browse/:id?params=`
**Why**: keeps each route's shape a direct, boring translation of the command it ports — no new
naming scheme to learn, and the frontend call sites are a 1:1 swap from `invoke` args to `fetch`
params.
**Alternative considered**: a single generic `/api/command/:name` dispatcher taking a JSON body.
Rejected — would need its own request/response envelope and error-shape convention invented from
scratch, for no benefit over eleven boring routes `axum::Router` already handles well.

### Error mapping: reuse the existing uniform `(StatusCode, String)` shape
**Decision**: every route maps `innertube::Error` to `502 Bad Gateway` with the error's message,
exactly like the existing `/api/search` route, except for failures the route itself can detect
before calling `innertube` (a missing/empty required param, an unsupported `category` value, a
desktop-local id passed to `/api/album` or `/api/playlist`) — those map to `400 Bad Request`.
**Why**: `innertube::Error` (`transport.rs:53-70`) has no `NotFound` variant — an unknown album or
artist id most likely surfaces as a generic `Other(String)` or a JSON-shape mismatch, not a value
this server can reliably distinguish from a transient upstream failure. Inventing a 404 on top of
that would be guessing at a distinction the error type doesn't actually make.
**Alternative considered**: a dedicated 404 for "not found" ids. Deferred — would need either a
change to `innertube::Error` (out of scope, see Non-Goals) or fragile message-sniffing; revisit if
a real caller needs to tell "not found" apart from "upstream hiccup."
**Confirmed during implementation**: an unknown album, artist, or playlist id doesn't even produce
an `Err` — YouTube's own `browse` endpoint responds `200` with a null/empty page for an id it
doesn't recognize, so there is no error to map at all for those three routes (specs updated to
match: a successful empty/null page, not a client error). `browse_grid` (`/api/browse/{id}`)
differs — an unknown id there does surface as a real upstream `4xx`, mapped to `502` same as any
other `innertube::Error` — so the two behaviors legitimately diverge per endpoint, not an
inconsistency to paper over.

### Desktop-local ids rejected with 400, not silently handled
**Decision**: the HTTP `/api/album/:id` and `/api/playlist/:id` routes do not special-case
`local::ALBUM_PREFIX`/`ARTIST_PREFIX`/`ON_REPEAT_ID` the way their Tauri-command counterparts do —
an id with one of those shapes is rejected with `400` before any `innertube` call is attempted.
**Why**: those ids only resolve against the desktop app's own SQLite library (`local.rs`), which
this server has no access to and the browser's own local-music feature
(`ui/src/lib/local/`, IndexedDB-backed) never produces or expects to resolve through this path.
Silently calling `innertube` with one of those ids would produce a confusing upstream error instead
of a clear one.
**Alternative considered**: teach the server about these id shapes and return a clean "not
supported here" message via the same 400 path either way. Equivalent user-facing outcome, slightly
more code; the implementer can pick either as long as the 400-not-innertube-error behavior holds.

## Risks / Trade-offs

- **[Risk]** `search_suggestions`'s own doc comment in `src-tauri/src/commands.rs` says "signed in"
  (`#54`), which may mean an anonymous session returns degraded results, an error, or works fine —
  unverified. → **Mitigation**: this is the one command in this batch that needs the same kind of
  early, standalone check `http-search-vertical-slice` task 2.2 did for `search_songs` before
  wiring the route — call it anonymously first, confirm real suggestions come back, before writing
  the HTTP route around it. If it genuinely degrades anonymously, that's a product decision
  (ship degraded suggestions vs. drop the route) to bring back to the user, not something to guess
  past here.
- **[Risk]** Eleven new routes sharing one `CorsLayer`/`AppState` is more surface than the single
  prior route; a mistake in one handler (e.g. an unhandled panic on a malformed continuation token)
  takes down the whole server process, not just that route. → **Mitigation**: no process-level
  isolation is being added in this change (would be a bigger architectural change than "port some
  routes") — each task's Verify step includes a deliberately malformed input (bad token, unknown
  category) specifically to catch a panic before it ships, same discipline the original slice's
  task 2.4 applied to the empty-query case.
