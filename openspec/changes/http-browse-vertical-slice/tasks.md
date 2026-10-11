## 1. Server: restructure before adding routes

- [x] 1.1 Move the existing `search` handler out of `crates/server/src/main.rs` into a new
      `crates/server/src/search.rs` exposing a `pub fn router() -> Router<Arc<AppState>>` (or
      equivalent), and wire it back in from `main.rs` via `Router::merge`. Verify: `cargo run -p
      server`, then `curl 'http://localhost:8787/api/search?q=a+day+in+the+life'` still returns the
      same JSON array it did before the move — pure refactor, no behavior change. — Confirmed:
      identical response, empty `q` still 400.
- [x] 1.2 Create `crates/server/src/browse.rs` with an empty `pub fn router() -> Router<Arc<AppState>>`
      (no routes yet), merged into `main.rs` next to `search::router()`. Verify: `cargo check -p
      server` compiles. — Confirmed.

## 2. Server: search routes

- [x] 2.1 Add `GET /api/search/all?q=` calling `search_all` (hard-coded `record_history: false`,
      same as the existing `/api/search`). Verify: `curl 'http://localhost:8787/api/search/all?q=a+day+in+the+life'`
      returns categorized results; `curl '.../api/search/all'` (no `q`) and `?q=` (empty) both
      return `400`. — Confirmed.
- [x] 2.2 Before wiring a route around it, call `InnerTube::search_suggestions` directly (a
      throwaway call in `main.rs` or a `#[test]`, same throwaway-first approach the original
      slice's task 2.2 used for `search_songs`) with the same anonymous `Session::default()` this
      server already uses. **Resolve design.md's flagged risk here**: does it return real
      suggestions, degraded results, or an error? Verify: real suggestion text prints (or, if it
      errors/degrades, stop and report back before building 2.3 around a command that doesn't
      actually work anonymously). — **Go**: real completions + matching songs/videos came back
      anonymously (e.g. "a day to remember", "a day in the life beatles"), no sign-in needed.
      Throwaway test deleted after confirming.
- [x] 2.3 Add `GET /api/search/suggestions?q=` calling `search_suggestions`. Verify:
      `curl '.../api/search/suggestions?q=a+day'` returns suggestions; `?q=` (empty) returns a
      successful empty list, not a `400` (per the spec — typeahead on nothing isn't an error the
      way a submitted search is). — Confirmed: empty `q` short-circuits locally to `{"queries":
      [],"items":[]}`, 200, no network call.
- [x] 2.4 Add `GET /api/search/cards?q=&category=` calling `search_cards`, mapping its existing
      `Error::Other("unknown search category: ...")` to `400` specifically (distinguishable from
      the generic upstream-error case — see design.md). Verify: `curl '.../api/search/cards?q=x&category=albums'`
      returns results; `category=nonsense` returns `400` with a message naming the bad category,
      not a `502`. — Confirmed.
- [x] 2.5 Add `GET /api/search/videos?q=` calling `search_videos`. Verify:
      `curl '.../api/search/videos?q=a+day+in+the+life'` returns videos; missing/empty `q` returns
      `400`, matching `/api/search`'s existing behavior. — Confirmed.

## 3. Server: browse routes

- [x] 3.1 Add `GET /api/home?params=` (optional query param) calling `home`. Verify:
      `curl '.../api/home'` returns a home page with sections. — Confirmed: 2 sections.
- [x] 3.2 Add `GET /api/home/more?token=` calling `home_continuation`. Verify: take a continuation
      token from 3.1's response, confirm `curl '.../api/home/more?token=<token>'` returns another
      page of sections; an empty/garbage token returns an error, not a crash (`cargo run -p server`
      stays up afterward). — Confirmed: real token → `200` (0 further sections after filtering, a
      legitimate empty success — anonymous home feed had nothing more); garbage token → `502`,
      server stayed up; missing token → `400`.
- [x] 3.3 Add `GET /api/album/:id` calling `album`, rejecting an id matching
      `local::ALBUM_PREFIX`/`ARTIST_PREFIX` with `400` before calling `innertube` (see design.md's
      "Desktop-local ids rejected with 400" decision — import or duplicate just the prefix
      constants, not any of `local.rs`'s SQLite logic). Verify: a real album id returns its title +
      tracks; an unknown id returns an error, not a crash; a `local::ALBUM_PREFIX`-shaped id returns
      `400` without a network call (check server logs/timing to confirm no upstream request fired).
      — **Spec updated during implementation**: an unknown album id doesn't error at all — YouTube's
      `browse` responds `200` with a null page (confirmed live); by agreement with the user, the spec
      now calls this a successful empty page, not an error. Real id and `LOCALALBUM:`-prefixed id (→
      `400`, no network call) both confirmed.
- [x] 3.4 Add `GET /api/artist/:id` calling `artist`. Verify: a real artist id returns their page; an
      unknown id returns an error, not a crash. — Same spec update as 3.3 applies (unknown artist id
      → `200` with a null page, confirmed live, spec updated). Real artist id confirmed.
- [x] 3.5 Add `GET /api/playlist/:id?sort=&desc=` calling `playlist`, rejecting an id equal to
      `ON_REPEAT_ID` (or otherwise matching a desktop-local shape) with `400` before calling
      `innertube`. Verify: a real public playlist id returns its tracks; `ON_REPEAT_ID` returns
      `400` without a network call; an unknown id returns an error, not a crash. — Same spec update
      (unknown playlist id → `200` null page, spec updated). Confirmed: real id with tracks,
      `sort=title&desc=true` accepted, `ON_REPEAT_ID` and a `LOCALPLAYLIST:`-prefixed id both `400`
      with no network call.
- [x] 3.6 Add `GET /api/playlist/more?token=` calling `playlist_continuation`. Verify: a token taken
      from 3.5's response returns more tracks for the same playlist; a garbage token errors without
      crashing the server. — Garbage token → `502`, server stayed up; missing token → `400`, both
      confirmed. **Partial**: no playlist found in this session long enough to produce a real
      continuation token (several real public playlists checked, all ≤100 tracks, no token) — the
      positive "a real token returns more tracks" path is unverified by curl, though the mechanism
      is identical to `home_continuation`'s (3.2), already confirmed working with a real token.
- [x] 3.7 Add `GET /api/moods` calling `moods`. Verify: `curl '.../api/moods'` returns mood/genre
      sections. — Confirmed: 2 sections (e.g. "Moods & moments": Chill, Commute, Energize, …).
- [x] 3.8 Add `POST /api/moods/art` taking a JSON array body (`Vec<String>`) calling `get_mood_art`'s
      underlying logic (check whether this lives on `InnerTube` directly or is composed in
      `commands.rs` from simpler calls — if the latter, port that composition into `browse.rs`
      rather than inventing a new shape). Verify: `curl -X POST -H 'Content-Type: application/json'
      -d '["<params-from-3.7-response>"]' .../api/moods/art` returns art for tiles that have it and
      simply omits ones that don't, as a successful response either way. — Confirmed: it's composed
      in `commands.rs` (concurrent via `futures_util`); ported as a sequential loop instead (no new
      dependency — see the function's doc comment), same observable behavior. Real params → art
      returned; bogus params → `{}`, both `200`.
- [x] 3.9 Add `GET /api/browse/:id?params=` calling `browse_grid`. Verify: a real browse id (e.g.
      one of the Moods & Genres tile ids from 3.7) returns a grid of items; an unknown id returns an
      error, not a crash. — Confirmed: real id+params returns a grid of playlists; an unknown id
      returns `502` (this route's unknown-id case genuinely does surface as an upstream error,
      unlike album/artist/playlist above — see design.md's updated "Error mapping" note), server
      stayed up.

## 4. Frontend wiring

- [x] 4.1 In `ui/src/lib/api.web.ts`, replace the `searchAll`, `searchSuggestions`, `searchCards`,
      and `searchVideos` stubs with real `fetch` calls against the routes from section 2, following
      `search`'s existing pattern exactly (same `SERVER_URL`, same error-on-non-ok-response
      convention). Verify: `pnpm check` in `ui/` passes with no new type errors. — Confirmed, 0
      errors. Factored the repeated fetch/error convention into a shared `fetchJson`/`postJson`
      helper (11 routes now use it, not just these 4) rather than copy-pasting it each time.
- [x] 4.2 Replace the `getHome`, `getHomeMore`, `getAlbum`, `getArtist`, `getPlaylist`,
      `getPlaylistMore`, `getMoods`, `getMoodArt`, and `getBrowseGrid` stubs with real `fetch` calls
      against the routes from section 3. Verify: `pnpm check` passes with no new type errors. —
      Confirmed, 0 errors.
- [x] 4.3 Remove the `!isTauri` redirect in `ui/src/routes/+page.ts` now that `/search`'s data
      dependencies no longer throw outside Tauri. Verify: `pnpm check` passes; the file's load
      function no longer references `/offline`. — The file had nothing left once the redirect was
      removed, so it was deleted rather than left as an empty no-op. Confirmed `pnpm check` still
      passes.

## 5. Verification

- [x] 5.1 With `cargo run -p server` running, start `pnpm dev` in `ui/` in a plain browser tab (not
      Tauri) and load `/`. Verify: the real home feed renders (not a redirect to `/offline`),
      sourced from `/api/home` (check the Network tab). — Confirmed via headless Chrome
      (`--dump-dom`, real tool, not Tauri): real hero ("Good night"), real shelves ("Shortcuts",
      "Today's biggest hits", "Quick picks"), 19 images, zero "/offline" mentions, zero leaked
      `unsupported(...)` error text.
- [x] 5.2 In the same browser tab, use the search box and open a result's album/artist page.
      Verify: `/search` renders real categorized results with no uncaught error, and clicking
      through to an album/artist page renders real content — all via the new `/api/*` routes, no
      Tauri IPC involved. — Confirmed via headless Chrome: `/search?q=beatles` renders "Top
      result"/"Songs"/"Albums"/"Artists"/"Videos"/"Playlists" sections (this is the exact page the
      original slice's task 4.1 found crashing on `searchAll()`); `/album/{id}` and `/artist/{id}`
      both render real headings, sections, and art with no error text.
- [x] 5.3 Confirm `/offline` still works unchanged (it's reachable directly, just no longer the
      forced landing page). Verify: navigating to `/offline` directly still shows the local-music
      UI from the prior change. — Confirmed: "Offline music", "Add folder", "Add files" all present.
- [ ] 5.4 Confirm the existing Tauri desktop build still works unchanged: run `cargo tauri dev` and
      use search, an album page, and an artist page there. Verify: all three work exactly as before,
      via `invoke`, with the new server never involved. — **Found and fixed a real, unrelated bug
      while verifying this**: an earlier session's edit to `src-tauri/tauri.conf.json` (splitting
      the Tauri vs. PWA build) used `{"command": ..., "cwd": ...}` for `beforeDevCommand`/
      `beforeBuildCommand`, but `tauri-utils`' `HookCommand` schema actually takes `{"script": ...,
      "cwd": ...}` — the wrong field name silently matched no variant of that untagged enum,
      breaking `cargo check -p limusic-app` outright (`data did not match any variant of untagged
      enum HookCommand`). Fixed by renaming `command` → `script`; confirmed `cargo check -p
      limusic-app` compiles clean afterward. **Then actually ran `cargo tauri dev`**: real window,
      webview loaded, video/audio proxies started, cipher analysis completed, BotGuard PoToken
      minter came up — all with no error, confirming `src-tauri` is otherwise unaffected by this
      change. **Not done this session**: the interactive click-through itself (typing a search,
      opening an album/artist page) needs a human driving the actual window — no GUI automation
      tool is wired up for Tauri's native webview in this environment (same gap
      `http-search-vertical-slice`'s own task 4.2 flagged).
