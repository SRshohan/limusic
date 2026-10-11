## Context

`crates/innertube` is already Tauri/webview/mpv-free by design (its own module doc: "knows nothing
about Tauri, webviews, mpv, or the OS... unit-testable against JSON fixtures with no network").
The existing Tauri command this slice mirrors is `search` in `src-tauri/src/commands.rs:23-32`:

```rust
pub async fn search(state: St<'_>, query: String, record_history: bool) -> Result<Vec<SongItem>, String> {
    let client = state.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
    let result = state.it.search_songs(client, &query, record_history).await.map_err(|e| e.to_string())?;
    Ok(result.items)
}
```

`state.it` is an `innertube::InnerTube` (constructed once at startup from a `Session` and a proxy),
`state.clients` an `innertube::Clients` (loaded once via `Clients::bundled()`, which parses a
compile-time-embedded `clients.json` — no runtime I/O, no Tauri). `METADATA_CLIENT` is the constant
`"WEB_REMIX"` (`crates/innertube/src/clients.rs:122`). `InnerTube::new(session: Session, proxy:
Option<&str>) -> Result<Self, Error>` (`transport.rs:190`) takes a plain `Session` — `Session::default()`
is a valid, anonymous (no cookie) session, which is exactly what an unauthenticated search needs,
matching how the desktop app already behaves signed out.

This is the first change against the "online" feature (separate from the already-built
`offline-local-music-pwa` change, which stays local-files-only). It intentionally does not touch
`src-tauri`, `AppState`, the orchestrator, or any other command — see proposal.md's scope.

## Goals / Non-Goals

**Goals:**
- Prove `crates/innertube` works unmodified behind a plain HTTP server.
- Prove the frontend can reach a non-Tauri backend for at least one real feature.
- Keep the new surface area small enough to be read and understood in one sitting by someone new
  to this codebase.

**Non-Goals:**
- Any other command, the orchestrator/stream-resolution pipeline, auth/cookies, a database, a
  websocket/event-push channel, or production-grade error handling/observability. All later,
  separate changes.
- Reusing `src-tauri`'s `AppState` shape — this slice's server state is just the two values
  `search` already needs (`InnerTube`, `Clients`), not a port of `AppState` itself.
- Hosting/deployment concerns (TLS, process supervision). This runs as `cargo run`, reachable on
  localhost, for development/demo purposes only.

## Decisions

### New crate `crates/server`, a plain `axum` binary
**Decision**: add `crates/server` (binary crate) to the workspace, depending on `innertube`
(path dependency, like `src-tauri` already does), `axum`, `tokio`, and `tower-http` (for the CORS
layer). It owns its own tiny state struct — not `AppState` from `src-tauri`.
**Why**: `src-tauri/Cargo.toml` already pulls `hyper`/`hyper-util`/`http-body-util` directly (manual
routing); `axum` sits on that same stack and gives routing/extractors for free, which matters more
once this server starts fanning out to more routes in later changes. A new crate (not adding routes
into `src-tauri` itself) keeps this change from touching Tauri-coupled code at all.
**Alternative considered**: bare `hyper` + `service_fn`, matching `audioproxy.rs`'s existing style.
Rejected for *this* crate specifically — it's the one that will keep growing routes across later
changes, where `axum`'s routing pays for itself immediately; `audioproxy.rs`'s hand-rolled approach
stays fine for the narrow proxy job it already does.

### Anonymous session only, no settings/database
**Decision**: the server constructs `innertube::Session::default()` once at startup (no cookie, no
visitorData persistence) and holds one `InnerTube` + `Clients::bundled()` behind `axum::extract::State`.
**Why**: `search_songs` doesn't require a signed-in session (the existing Tauri command passes
`record_history: bool` precisely because recording search history is the only signed-in-only
behavior here) — matching the desktop app's own signed-out behavior keeps this slice from needing
any persistence layer at all.
**Alternative considered**: fetching an anonymous `visitorData` up front, the way the desktop app's
first-run bootstrap does. Deferred — `search_songs` works without it; see Open Questions.

### Route shape: `GET /api/search?q=<query>`
**Decision**: a single query parameter `q`, hard-coded `record_history: false` (there is no session
to record history against yet), response body is the same `Vec<SongItem>` shape the Tauri command
already returns (its `Serialize` impl is unchanged — reusing `innertube`'s own model types directly
as the HTTP response body, via `axum::Json`).
**Why**: keeps the response shape identical to what `ui/src/lib/api.ts` already expects from the
Tauri path, so the frontend's existing `SongItem` TypeScript type needs no changes.

### CORS: permissive, development-scoped
**Decision**: `tower_http::cors::CorsLayer::permissive()` (or an explicit allow-origin for
`http://localhost:5183`), applied to the whole router.
**Why**: this slice is a local development/demo artifact, not a production deployment; the later
PWA-polish change is where real origin/session-scoped CORS gets decided alongside auth.

### Frontend: a conditional branch inside the existing `search()` function
**Decision**: `ui/src/lib/api.ts`'s `search(query, recordHistory)` checks `isTauri` (from
`ui/src/lib/platform.ts`, already built in the prior change) and either calls `invoke('search', ...)`
(unchanged) or `fetch` against the new server's `/api/search` route, parsing the same JSON shape
into the same return type.
**Why**: this is the smallest possible frontend change that proves the round trip — every call site
of `search()` elsewhere in the app keeps working unmodified, whichever path it takes.
**Alternative considered**: a parallel `search()` implementation in a new module. Rejected — would
leave two divergent call sites for callers to choose between, which this slice has no reason to
introduce yet.

## Risks / Trade-offs

- **[Risk]** Hard-coding the server's base URL in the frontend (e.g. `http://localhost:8787`) is
  fragile once the server needs to run somewhere other than localhost. → **Mitigation**: acceptable
  for this slice (explicitly a development/demo artifact per Non-Goals); a real configuration
  story is a later change's problem once there's more than one route to configure.
- **[Risk]** No request timeout/error-shape consistency between the two `search()` paths (Tauri's
  `Result<_, String>` vs. an HTTP error body) could surprise a caller. → **Mitigation**: scenario
  "a missing or empty query" in the spec pins down the one error case this slice actually needs to
  handle; broader error-shape consistency is deferred to the change that ports more commands.

## Open Questions

- ~~Should the anonymous session also fetch a real `visitor_data` up front...~~ **Resolved** during
  task 2.2: `Session::default()` with no `visitor_data` returns real, full search results (20 songs
  for a test query) with no sign of rejection. No startup `visitor_data` fetch needed.
