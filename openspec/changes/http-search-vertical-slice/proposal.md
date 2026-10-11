## Why

The plan to turn Limusic into a self-hosted server + PWA (see
`/Users/sohanurrahman/.claude/plans/snug-pondering-kahan.md`, Phases 2–4) carries real architectural
risk: a new Rust HTTP server reusing the existing `crates/innertube` client, and a frontend that
talks to it instead of Tauri `invoke`. Rather than building a whole phase's worth of backend before
anything is testable, this change proves the riskiest assumption — that `crates/innertube` really
is reusable as-is behind a plain HTTP server, and that the frontend can call it — with the smallest
possible real, working path: one search request, end to end, through a new server. It also doubles
as an onboarding task: whoever builds it comes out the other side understanding `AppState`'s shape,
how a command currently calls into `crates/innertube`, and where the real complexity (orchestration,
cipher) still lives, without having to absorb the whole backend first.

## What Changes

- Add a new Rust binary crate, a thin `axum` HTTP server, that:
  - Bootstraps an anonymous InnerTube session (`innertube::Session::default()` + `Clients::bundled()`
    + `InnerTube::new`) — no cookies, no login, mirroring how the desktop app already works signed
    out.
  - Exposes exactly one route, `GET /api/search?q=<query>`, returning the same JSON shape
    (`Vec<SongItem>`) the existing Tauri `search` command already returns.
  - Sends permissive CORS headers so the SvelteKit dev server (`http://localhost:5183`) can call it
    cross-origin in development.
- `ui/src/lib/api.ts`'s existing `search()` function gains a parallel path: outside Tauri
  (`isTauri` from the prior change's `platform.ts`), it calls the new HTTP route via `fetch`
  instead of `invoke('search', ...)`. Inside Tauri, nothing changes.
- Nothing else. No other command is ported, no event/push channel, no streaming, no auth, no
  database, no orchestrator. Those are separate, later changes.

## Capabilities

### New Capabilities
- `search-http-api`: the server-side behavior of resolving a text query to a list of matching
  songs over a plain HTTP request, independent of any specific client.

### Modified Capabilities
_None — the existing Tauri `search` command and its behavior are unchanged; this adds a second,
independent way to reach the same kind of result._

## Impact

- New crate under `crates/` (added to the root `Cargo.toml` workspace `members`), with `axum` and
  `tower-http` as new dependencies. `crates/innertube` itself is unmodified — reused as a plain
  library dependency.
- `ui/src/lib/api.ts` gains a small conditional branch in one function; no other frontend file
  changes.
- Nothing in `src-tauri` changes. The desktop build is unaffected.
