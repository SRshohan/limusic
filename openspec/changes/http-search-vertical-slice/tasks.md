## 1. Orientation (read-only, no code)

- [x] 1.1 Read `crates/innertube/src/lib.rs` (the crate's module doc + re-exports) and
      `crates/innertube/src/transport.rs`'s `Session`/`InnerTube::new` (around line 190). Verify:
      you can explain in one sentence what `Session::default()` gives you and why `InnerTube::new`
      takes a `Session` + an optional proxy string. — `Session::default()` is an anonymous session
      (no cookie/visitor_data); `InnerTube::new` takes a proxy string because `reqwest` snapshots
      proxy config at client-build time, not per-request.
- [x] 1.2 Read `src-tauri/src/commands.rs`'s `search` function (near the top of the file, ~line 23)
      and `crates/innertube/src/clients.rs`'s `METADATA_CLIENT` constant and `Clients::bundled()`.
      Verify: you can trace, by hand, the exact call path from "a query string" to
      "a `Vec<SongItem>`" with no code running yet — this is the path the new server reimplements.
      — query → look up `WEB_REMIX` in the bundled `Clients` registry → `InnerTube::search_songs`
      → `search_raw` (POST, `FILTER_SONG`) → `parse_search` → `Vec<SongItem>`.

## 2. The server crate

- [x] 2.1 Create `crates/server/Cargo.toml` (binary crate) depending on `innertube` (path
      dependency, see how `src-tauri/Cargo.toml` depends on it for the pattern), `axum`, `tokio`
      (with the `rt-multi-thread`/`macros` features), and `tower-http` (`cors` feature). Add
      `crates/server` to the root `Cargo.toml`'s workspace `members`. Verify: `cargo check -p server`
      compiles with an empty `main.rs` (just a `fn main() {}`). — Note: axum 0.8's transitive deps
      (`time`, `darling`/`serde_with`) need rustc ≥1.88; this machine's Homebrew `rustc` is 1.87, but
      a rustup-managed 1.91 toolchain is also installed (`rustup which cargo`) — used that instead.
      Confirmed the whole workspace, including `src-tauri`, still builds clean under it.
- [x] 2.2 In `crates/server/src/main.rs`, construct one `innertube::InnerTube` from
      `innertube::Session::default()` and `None` (no proxy), and one `innertube::Clients::bundled()`.
      Write a throwaway call to `search_songs` with a hard-coded query (e.g. `"a day in the life"`)
      printed to stdout, run it with `cargo run -p server`, and confirm real YouTube Music results
      print. **This is the riskiest step in the whole slice — if it doesn't return results
      anonymously, resolve the design doc's Open Question (fetching a `visitor_data` first) before
      continuing.** Verify: real song titles/artists print to the terminal. — **Go**: 20 real
      results (The Beatles, Ramallah, etc.) with no `visitor_data` at all — design.md's Open
      Question is resolved, no further work needed there.
- [x] 2.3 Replace the throwaway call with an `axum::Router` serving `GET /api/search?q=<query>`,
      with the `InnerTube`/`Clients` pair behind `axum::extract::State`. Return the query's
      `Vec<SongItem>` as `axum::Json`. Verify: `cargo run -p server`, then
      `curl 'http://localhost:<port>/api/search?q=a+day+in+the+life'` returns a JSON array of songs.
      — Confirmed: real `SongItem` JSON (video_id/title/artists/album/thumbnail/…) on port 8787.
- [x] 2.4 Handle the missing/empty-query case with a 4xx response instead of panicking or
      returning an empty success list. Verify: `curl 'http://localhost:<port>/api/search'` (no `q`)
      returns a client error status, not a 200 or a crash. — Confirmed: both no `q` and `q=`
      (empty) return `400`.
- [x] 2.5 Add `tower_http::cors::CorsLayer` to the router, scoped at least to
      `http://localhost:5183` (the SvelteKit dev server's fixed port — see `ui/vite.config.ts`).
      Verify: see task 4.2 (the actual cross-origin browser request). — Confirmed directly via
      `curl -H "Origin: http://localhost:5183"`: response carries
      `access-control-allow-origin: http://localhost:5183`. Full browser round-trip still pending
      as part of 4.1.

## 3. Frontend wiring

- [x] 3.1 In `ui/src/lib/api.ts`, find the existing `search()` function (the one that currently
      calls `invoke('search', ...)`). Add a branch using `isTauri` from `ui/src/lib/platform.ts`:
      when false, `fetch` the new server's `/api/search?q=...` route and parse the JSON into the
      same return shape the Tauri path returns. Leave the Tauri branch untouched. Verify:
      `pnpm check` in `ui/` passes with no new type errors. — Confirmed 0 errors. `SongItem`'s TS
      fields already match the Rust struct's serde field names exactly, so the fetched JSON needs
      no reshaping — it satisfies the existing `SongItem[]` type as-is.

## 4. Verification

- [x] 4.1 With `cargo run -p server` running, start the frontend dev server (`pnpm dev` in `ui/`)
      in a plain browser tab (not the Tauri dev build) and use the app's search box. Verify: real
      results appear in the UI, sourced from the new server (check the Network tab for the
      `/api/search` request, not a Tauri IPC call). — **Narrowed, by agreement**: the real
      `/search` page also calls `searchAll()` in the same `Promise.all`, uncaught, which still
      throws outside Tauri (`Cannot read properties of undefined (reading 'invoke')`) and fails the
      whole page — `searchAll` is a different, out-of-scope command (design.md's Non-Goals). Proved
      the actual round trip instead with a throwaway static page served from the dev server's own
      `http://localhost:5183` origin, doing the same cross-origin `fetch` `api.search()`'s browser
      branch does: real response, 20 results, real titles/artists. Deleted after confirming.
      Porting `searchAll` (and the rest of the ~90 commands) so the real page works end to end is
      explicitly the next, separate change.
- [ ] 4.2 Confirm the existing Tauri desktop build still works unchanged: run `cargo tauri dev`
      (or the project's existing dev command) and use search there. Verify: it still works exactly
      as before, via `invoke`, with the new server never involved. — **Partially confirmed this
      session, rest needs a human**: `mpv` *is* installed on this machine (Homebrew) — my first
      attempt failed only because my sandboxed shell doesn't source `brew shellenv`
      (`LIBRARY_PATH` was empty); with `LIBRARY_PATH=/opt/homebrew/lib` set explicitly,
      `cargo build -p limusic-app` compiles **and links** cleanly. `cargo tauri` CLI still isn't
      installed in this environment, and actually running the app + clicking the search box needs
      a GUI session — genuinely needs a human with a normal terminal (where this isn't an issue at
      all) to finish.
