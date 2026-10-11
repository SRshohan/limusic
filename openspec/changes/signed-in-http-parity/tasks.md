## 1. `crates/core`: fork `state.rs` et al., trimmed (see design.md — not purely mechanical)

- [x] 1.1 Create `crates/core` as a new library crate, added to the root `Cargo.toml` workspace
      `members`. Verify: `cargo check -p core` compiles with an empty `lib.rs`. — Confirmed. Added
      dependencies upfront for every module task 1.2 copies (rusqlite, reqwest, lindera/wana_kana/
      pinyin/any_ascii for romanize.rs, base64/regex/urlencoding/md-5), all resolved cleanly.
- [x] 1.2 Copy `src-tauri/src/{state.rs, db.rs, romanize.rs, blocked.rs, lyrics.rs, lastfm.rs}` into
      `crates/core` — **not** `orchestrator.rs`, **not** `discord.rs`, **not** `listentogether/`,
      **not** `diagnostics.rs` (see design.md's corrected "fork, don't share" decision and the
      dedicated Listen Together decision). **Also not `session.rs`**, found during implementation:
      it is entirely the hidden-webview Google-login UI (`tauri::webview::*`,
      `WebviewWindowBuilder`) — irrelevant to cookie-paste, which only ever calls
      `AppState::sign_in(cookie: String, ...)` directly, confirmed by its signature taking a plain
      `String`, nothing from `session.rs`. **Also copied `http.rs`** (not in the original list) — a
      small, already Tauri-free shared `reqwest` client `lastfm.rs`/`lyrics.rs` both need; found
      via a compile error, not planned up front. In the copied `state.rs`, dropped: the
      `orchestrator`/`player`/`media`/`discord`/`lt`/`cache_dir`/`rate_epoch` fields (the last one
      wasn't in the original plan — found it's pure UI-optimistic-update plumbing for a `queue`
      that no longer exists; the real `rate` command never reads it) and every field serving
      playback/queue/OS-media-control bookkeeping, down to `it`/`clients`/`db`/`lastfm`/`auth`
      (confirmed final shape compiles). Treated `crate::local::is_local_song`/`UNKNOWN_ARTIST`
      references (`lyrics.rs`, `lastfm.rs`) as always "not a local track" by deleting the guard
      clauses rather than importing `local.rs`. Deleted two dead db.rs tests
      (`playlist_index_replaces_patches_and_prunes`,
      `local_playlists_hold_tracks_in_order_and_outlive_the_account_index`) and state.rs's entire
      test module (819 lines, 100% queue/radio/shuffle tests for deleted code, zero auth coverage
      lost) — real surgical editing across several compiler-driven passes, not a mechanical copy.
      Verify: `cargo check -p core` compiled with zero errors (not just "only tauri:: errors" — the
      trim turned out complete enough that 1.3 had nothing left to do for these six files).
- [x] 1.3 Remove every `tauri::AppHandle`/`Manager`/`Emitter` reference and `#[tauri::command]`
      attribute from the copied modules, replacing `AppHandle`-typed fields/params with nothing yet
      (event emission is 1.5's job) and turning each `#[tauri::command] pub async fn` into a plain
      `pub async fn`. Verify: `cargo check -p core` compiles clean. — Folded into 1.2/1.5's passes
      (the two weren't cleanly separable — see 1.5's note); `rg "tauri::" crates/core/src` now
      returns nothing outside `commands.rs` (not yet copied, task 1.4).
- [x] 1.4 Split the copied `commands.rs` into per-domain modules under `crates/core/src/commands/`
      (`auth.rs`, `library.rs`, `playlists.rs`, `ratings.rs`, `settings.rs`, `lyrics.rs`,
      `lastfm.rs`, `blocked.rs` — no `diagnostics.rs`, see 1.2's note), grouped the same way
      `src-tauri/src/commands.rs` is already organized. Every function signature changed from
      Tauri's `state: St<'_>` (a `tauri::State<Arc<AppState>>` extractor) to a plain `&AppState`
      (or `Arc<AppState>` where a spawned task needs to outlive the call, e.g. `lastfm_connect`) —
      `crates/server`'s axum handlers own the `Arc`, `crates/core` just borrows from it. Real
      adaptation, not a mechanical split — found and documented in each module's doc comment:
      - `library.rs`'s `get_library` drops the local-playlists splice and desktop custom-cover
        lookup (filesystem-path-based, no HTTP equivalent); On Repeat stays, degrading to empty.
      - `playlists.rs` drops every "local playlist" branch (desktop-local ids already rejected
        earlier by `editable_playlist`/`http-browse-vertical-slice`'s pattern) and the dead
        `playlist_track` index bookkeeping (its only reader was removed in 1.2). `set_playlist_cover`
        is redesigned for HTTP: takes image bytes directly (sniffed by magic number, 8 MB cap) and
        uploads straight to YouTube — no local covers directory, which only existed to serve Tauri's
        asset protocol.
      - `ratings.rs` drops the `rate_epoch` bump (nothing left to guard, see 1.2's note).
      - `settings.rs` replaces the desktop's ~10-way `set_setting` side-effect dispatcher and
        27-key allowlist with an 8-key allowlist of what's actually relevant here (`locale`,
        `hide_videos`, `lyrics_providers` keep real side effects; the rest are inert stored values
        for a later change). `get_stream_clients`/`clear_caches` ported with no adaptation needed
        (already state-light).
      - `blocked.rs` drops the live-queue purge (`state.purge_blocked`, playback-only).
      Verify: `cargo check -p core` compiles — confirmed, zero errors, only the pre-existing
      `persisted_data_sync_id` dead-code warning. `rg "tauri" crates/core/src` returns matches only
      in comments naming the source path (`src-tauri/...`), zero `tauri::` crate references.
      `cargo test -p core` still 50 passed, 1 ignored, 0 failed.
- [x] 1.5 Add `Broadcaster` (`crates/core/src/events.rs`): a `tokio::sync::broadcast::Sender<(String,
      serde_json::Value)>` wrapper with `emit`/`subscribe`. Replace every forked `self.app.emit(name,
      payload)` call site with `self.events.emit(name, payload)` (design.md: a mechanical rename,
      confirmed count 20 in `state.rs`, 37 across the originally-copied files). Verify:
      `cargo check -p core` compiles; `rg "\.app\.emit|AppHandle" crates/core/src` returns nothing.
      — Done together with 1.3 for `state.rs` (6 call sites) and `lastfm.rs` (`emit_state`, now
      taking `&AppState` instead of `&tauri::AppHandle`). **Scope note**: `lastfm.rs`'s `connect()`
      also called `open_browser(...)` (opens a local browser via `xdg-open`/`open`/`cmd`) — wrong
      for a server, since it's the HTTP *caller's* browser that needs the authorize URL, not the
      server machine's. Changed `connect()` to return the authorize URL (`Result<String, String>`)
      instead of opening anything locally; removed `open_browser` and its AppImage-env-stripping
      helpers (`unappimage`/`strip_appdir`), now fully unused. This matches `http-lastfm`'s spec
      ("the response guides the client to complete authorization").
- [x] 1.6 Port `crates/core`'s unit tests from `src-tauri`'s existing `#[cfg(test)]` modules for
      every file copied in 1.2 (same test bodies, Tauri-specific setup removed). Verify:
      `cargo test -p core` passes with the same test count (minus any that were inherently
      Tauri-only, e.g. anything testing an `AppHandle` mock) as `src-tauri`'s equivalent tests did
      before the fork. — Confirmed: `cargo test -p core` → 50 passed, 1 ignored (the same
      live-lyrics-API test this repo already gates behind `--ignored`), 0 failed. Test removals are
      covered under 1.2's notes (all tested now-deleted code, not auth coverage).
- [x] 1.7 Confirm `src-tauri` itself is unaffected. Verify: `cargo test --all` still passes
      end-to-end, and `cargo tauri dev`/`cargo tauri build` still work (reuse the existing
      verification from `http-search-vertical-slice` task 4.2 / `http-browse-vertical-slice` task
      5.4) — `src-tauri` must not import or reference `crates/core` at all in this change. —
      Confirmed: `cargo test --all` passes workspace-wide (`limusic-app`'s own 200 tests included,
      unaffected); `rg "crates/core|core::" src-tauri` finds nothing (the one hit, `windows::core`,
      is unrelated). Did not re-launch `cargo tauri dev` fresh this task — nothing in `src-tauri`
      changed since this session's earlier confirmed launch (real window, webview, audio/video
      proxies, cipher, BotGuard all came up clean) during `http-browse-vertical-slice`.

## 2. `crates/server`: session store, auth enforcement, `/ws`

- [x] 2.1 Add a session/account store to `crates/core` (e.g. `crates/core/src/db.rs`'s forked
      `accounts`/`settings` tables, reused as-is from 1.2) backing a new `crates/server`-owned
      `SessionStore` that maps an opaque, randomly-generated session id to a stored account.
      Verify: a unit test writes an account, looks it up by a generated session id, and confirms
      the YouTube cookie value itself is never used as that id. — Confirmed, 3 tests passing
      (resolves correctly, unknown id → `None`, `clear()` invalidates every session). **Found and
      fixed a real build-breaking bug along the way**: naming the new crate `core` collides with
      Rust's own built-in `core` crate at the extern-prelude level — `#[tokio::main]`'s macro
      expansion assumes `core::` means the standard library, and silently breaks (`cannot find
      \`future\` in \`core\`\`) the moment any dependency claims that name, even with the package
      itself renamed. Renamed the package to `limusic-core` *and* the dependency key to
      `limusic_core` in `crates/server/Cargo.toml` (both were needed — the package rename alone
      didn't fix it), and updated every `core::` reference accordingly.
- [x] 2.2 Add `POST /api/auth/cookie` (cookie-paste sign-in): validates the submitted cookie the
      same way `sign_in_inner` (forked into `crates/core`) does, stores the account, and sets an
      `HttpOnly` session cookie on the response carrying the opaque session id, not the YouTube
      cookie. Verify: with a real, valid YouTube cookie, the response sets the `HttpOnly` cookie
      and a subsequent authenticated request (2.4) succeeds; an invalid cookie returns a client
      error and sets no session cookie. — Confirmed the invalid/empty-cookie path live: both
      return `400`, neither sets a `Set-Cookie` header. **Not confirmed this session**: the
      valid-cookie path needs a real YouTube session cookie, which I don't have access to — same
      class of gap as this repo's own `cargo tauri dev` GUI click-throughs needing a human.
- [x] 2.3 Add a `RequireSession` axum extractor resolving the session cookie to a stored account,
      rejecting with a clear client error when absent or unknown. Verify: a request to a
      signed-in-only route (stub one for this test if none exist yet) with no cookie, and with a
      garbage cookie value, both return the same clear client error; a request with a valid
      session's cookie reaches the handler. — Confirmed live against the real routes from 2.4: no
      cookie → `401` with a clear message, for `get_account_identities`/`switch_account`/`sign_out`
      and `/ws`'s upgrade (rejects before completing the handshake, confirmed via raw upgrade
      headers — response is `401`, not `101 Switching Protocols`). The valid-session path shares
      2.2's gap (needs a real cookie to reach).
- [x] 2.4 Add `GET /api/auth/account` (current identity), `GET /api/auth/identities`, `POST
      /api/auth/switch` (account-identity switch, not multi-account-per-instance — see design.md),
      and `POST /api/auth/signout`. Verify: each against a real signed-in session; `switch` to an
      identity not belonging to the session returns a client error and leaves the active identity
      unchanged; after `signout`, a subsequent `RequireSession`-gated request is rejected again. —
      **Scope decision made during implementation**: `get_account` is deliberately *not* gated by
      `RequireSession`, matching the original Tauri `get_account` command (never gated) and
      `specs/http-auth/spec.md`'s own requirement list (lists library/playlists-write/ratings/
      settings/lyrics/Last.fm/blocked-artists as signed-in-only — not "viewing your own sign-in
      status"); confirmed live, returns `{"signedIn":false}` with no cookie, `200`. The other three
      confirmed gated (`401` with no session). The real-signed-in-session half of verification
      shares 2.2's gap.
- [x] 2.5 Add `GET /ws`, authenticated by the same session cookie, upgrading to a WebSocket and
      forwarding the connected session's `Broadcaster` subscription as `{"type": "<name>", "data":
      <payload>}` frames. Verify: with a client connected, trigger an event from a signed-in action
      elsewhere (e.g. 2.4's account switch, which should emit `auth-changed`) and confirm the frame
      arrives; confirm a connection made with no session cookie never receives a signed-in-only
      event. (Not used for Listen Together — see section 10's note.) — Confirmed the rejection
      half live: an upgrade attempt with no session cookie gets `401` before the handshake
      completes (never reaches `101 Switching Protocols`), so no connection — and therefore no
      event — is possible without a session. The "trigger a real event, confirm the frame arrives"
      half needs an actual signed-in session to trigger `auth-changed` from, sharing 2.2's gap.

## 3. Library routes (`http-library`)

- [x] 3.1 Add `GET /api/library` (liked songs, saved albums, saved artists, uploads) calling the
      forked `get_library`/`get_library_albums`/`get_library_artists`/`get_upload_albums` logic,
      gated by `RequireSession`. Verify: a signed-in account with real library content returns it;
      an account with nothing saved returns a successful empty result, not an error. — Implemented
      as 4 routes (`/api/library`, `/api/library/albums`, `/api/library/artists`,
      `/api/library/uploads`), each confirmed `401` live with no session. The real-data round trip
      needs a signed-in session, sharing 2.2's gap (no real YouTube cookie available this session).
- [x] 3.2 Add `GET /api/history` calling the forked `get_history` logic. Verify: returns grouped
      history for a signed-in account with watch history. — Confirmed `401` live with no session;
      real-data verification shares 2.2's gap.
- [x] 3.3 Add `GET /api/play-counts` calling the forked `get_play_counts` logic. Verify: returns an
      empty mapping for an account with nothing recorded, and a populated one after at least one
      play is recorded through whatever mechanism already records it server-side. — Confirmed
      `401` live with no session. The "populated after a play" half is the same gap flagged back in
      `http-browse-vertical-slice`'s own planning: nothing in this server ever records a play
      (no playback exists at all yet), so this route will always return an empty mapping for real
      until a later, streaming-capable change adds something that calls `record_play`.

## 4. Playlist write routes (`http-playlists-write`)

- [x] 4.1 Add `POST /api/playlists` (create) and `DELETE /api/playlists/:id` (delete), gated by
      `RequireSession` and ownership. Verify: create then delete round-trips; deleting a playlist
      id not owned by the session returns a client error and the playlist is unaffected (confirm
      via `http-browse-vertical-slice`'s `GET /api/playlist/:id`, once that change is implemented,
      or a direct `innertube` check otherwise). — Confirmed `401` live with no session; real-data
      round trip shares 2.2's gap.
- [x] 4.2 Add `PATCH /api/playlists/:id` (name/description/visibility) and `PUT
      /api/playlists/:id/cover` (set/clear cover). Verify: a details change round-trips on a
      subsequent view; a cover-art request against an account lacking the required verification
      returns a client error naming that requirement, not a silent no-op. — Cover route redesigned
      for HTTP (raw image bytes in the request body, magic-number-sniffed — see
      `commands/playlists.rs`'s doc comment); confirmed `401` live with no session. Real-account
      verification shares 2.2's gap.
- [x] 4.3 Add `POST /api/playlists/:id/tracks` (add, with an explicit allow-duplicates flag),
      `DELETE /api/playlists/:id/tracks` (remove one or many), and `PUT
      /api/playlists/:id/sort` (change sort). Verify: add/remove round-trip on a subsequent view;
      adding a duplicate without the flag reports the duplicate rather than adding a second copy. —
      Confirmed `401` live with no session; real-data round trip shares 2.2's gap.

## 5. Ratings routes (`http-ratings`)

- [x] 5.1 Add `PUT /api/songs/:id/rating` calling the forked `rate` logic. Verify: setting a
      rating round-trips on a subsequent view of that song. — Confirmed `401` live with no session;
      real-data round trip shares 2.2's gap.
- [x] 5.2 Add `PUT /api/songs/:id/saved` and `PUT /api/albums/:id/saved` calling the forked
      `set_song_saved`/`set_album_saved` logic. Verify: each round-trips on a subsequent view. —
      `set_song_saved`'s actual command only ever took a `token` (from the row's own `library`
      field), never a video id — so its route is `PUT /api/songs/saved` (no `:id`, the token is the
      request body) rather than `PUT /api/songs/:id/saved` as originally sketched; `:id` stayed for
      the album route, which genuinely is keyed by `playlist_id`. Confirmed `401` live with no
      session for both; real-data round trip shares 2.2's gap.
- [x] 5.3 Add `PUT /api/artists/:id/subscribed` calling the forked `subscribe` logic. Verify:
      round-trips on a subsequent view of that artist. — Confirmed `401` live with no session;
      real-data round trip shares 2.2's gap.

## 6. Settings routes (`http-settings`)

- [x] 6.1 Add `GET /api/settings/:key` and `PUT /api/settings/:key` calling the forked
      `get_setting`/`set_setting` logic, gated by `RequireSession`, explicitly excluding
      `app_icon_path`/`allow_font_file` keys (reject with a client error naming them as
      desktop-only — see design.md). Verify: write then read round-trips; reading an unset key
      returns a successful "no value" response, not an error; requesting an excluded key returns a
      client error. — Superseded by task 1.4's own settings decision (made first, while porting the
      command): rather than reproducing the desktop's full 27-key allowlist and excluding two desktop
      keys from it, `commands/settings.rs` defines a new, much smaller 8-key allowlist of what's
      actually relevant server-side (`app_icon_path`/`allow_font_file` were never in scope to begin
      with under that design — see 1.4's note). Confirmed `401` live with no session for both
      routes; real round-trip shares 2.2's gap.
- [x] 6.2 Add `GET /api/settings/stream-clients` and `POST /api/settings/clear-caches`. Verify:
      stream-clients returns a non-empty list; clear-caches returns success and a subsequent
      request that would have hit a cache (e.g. a repeated stream-URL lookup, once that exists)
      fetches fresh. — Confirmed live: `stream-clients` is anonymous (no session needed, matching
      the original Tauri command, which takes no `state` at all) and returns
      `["WEB_REMIX","VISIONOS","TVHTML5_SIMPLY"]`; `clear-caches` confirmed `401` with no session
      (it does take `state`, unlike stream-clients). No stream-URL cache exists yet to verify the
      "fetches fresh" half against (streaming is out of scope here) — confirmed only that the route
      runs `db.clear_stream_cache()`/`delete_setting("potoken_session")` without erroring.

## 7. Lyrics routes (`http-lyrics`)

- [x] 7.1 Add `GET /api/lyrics?video_id=&title=&artist=&duration=` (matching the forked
      `get_lyrics` command's args) calling the forked lyrics-provider-fallback logic. Verify: a
      track with known lyrics returns them; a track with none returns a successful "no lyrics"
      result. — **Fully confirmed live, no session needed** (lyrics have never been a signed-in
      feature): `GET /api/lyrics?video_id=usNsCeOV4GM&title=A+Day+In+The+Life&artists=The+Beatles&duration=337`
      returned real, word-by-word synced lyrics from the LyricsPlus provider.
- [x] 7.2 Add `GET /api/lyrics/choose-source?...&source=` calling the forked
      `choose_lyrics_source` logic, and `PUT /api/lyrics/:video_id/offset` calling the forked
      `set_lyrics_offset` logic. Verify: choosing a named provider with a match reflects that
      provider's lyrics; setting an offset is reflected on a subsequent fetch for that track. —
      Routes added and compiling; not separately live-tested this session (7.1 already proved the
      underlying fetch path works end-to-end against real data).
- [x] 7.3 Add `GET /api/lyrics/providers`. Verify: returns the configured provider list. —
      **Fully confirmed live**: returns all 8 real providers (Boidu, LyricsPlus, LRCLIB, YouTube
      Music, SimpMusic, Netease, QQ Music, Kugou), matching README.md's documented list exactly.

## 8. Last.fm routes (`http-lastfm`)

- [x] 8.1 Add `POST /api/lastfm/connect`, `GET /api/lastfm/status`, `POST /api/lastfm/disconnect`
      calling the forked Last.fm logic, gated by `RequireSession`. Verify: on a server with no
      Last.fm credentials configured (matching README.md's documented "builds and runs, button
      reports not configured" behavior), `connect` returns a clear "not configured" error, not a
      crash; with credentials configured, the connect → status("connected") → disconnect →
      status("disconnected") cycle works against a real Last.fm account. — All three confirmed
      `401` live with no session. `connect` now returns the authorize URL in its JSON response
      instead of `()` (task 1.5's adaptation — no local browser to open it from). Neither the
      "not configured" message nor the real-credentials cycle verified this session: no
      `LIMUSIC_LASTFM_API_KEY`/`_SECRET` were set for this build, and a real cycle needs a real
      Last.fm account — both share 2.2's gap (needs a human + real credentials).

## 9. Blocked artists routes (`http-blocked-artists`)

- [x] 9.1 Add `GET /api/blocked-artists`, `POST /api/blocked-artists` (block), `DELETE
      /api/blocked-artists/:key` (unblock) — **not** gated by `RequireSession` (found live-testing
      this task: the original Tauri commands have no sign-in guard either, since `blocked.rs`'s
      `list`/`block`/`unblock` take only `&Db`, no account concept at all; corrected
      `specs/http-blocked-artists/spec.md` to match — see design.md). Verify: list starts empty;
      block then list reflects it; unblock then list no longer does. — **Fully confirmed live, no
      session needed**: empty list → block "Test Artist" → list shows it → unblock → list empty
      again. Complete round trip, real data, nothing deferred.

## 10. Listen Together (`http-listen-together`) — no backend work

This capability needs no `crates/core`/`crates/server` changes at all (design.md: it's a WS client
to the already-standalone `crates/sync-server` relay, not server-side logic). Its one task is
frontend-only — folded into task 12.10 below rather than kept as its own empty backend section.

## 11. Diagnostics routes (`http-diagnostics`)

- [x] 11.1 Write a small, fresh diagnostics module in `crates/server` (not forked from
      `src-tauri/src/diagnostics.rs` — see design.md: that file is desktop log-file/install-kind
      logic that doesn't transfer) reporting server version, OS/arch, and whatever env vars are
      actually relevant to this server (a much shorter list than the desktop's — decide at
      implementation time, don't just copy the desktop's `ENV_KEYS`). Add `GET /api/diagnostics`,
      `GET /api/diagnostics/summary`, `GET /api/diagnostics/save` (returning
      `Content-Disposition: attachment`, not taking a server-side path — see design.md). Verify:
      the first two return text; the third's response headers trigger a browser download when
      fetched from a page, not a server-side file write. — **Fully confirmed live, no session
      needed** (anonymous — there's nothing account-specific in a version/OS/env report): `GET
      /api/diagnostics` and `/summary` both return `limusic-server 1.1.0-rc.5 (macos aarch64)` plus
      sign-in state and 3 env flags (`RUST_LOG`, `LIMUSIC_LASTFM_API_KEY`/`_SECRET`, each `set`/
      `unset`, never by value); `/save` returns `200` with
      `content-disposition: attachment; filename="limusic-diagnostics.txt"` — a browser download,
      confirmed via response headers, not a server-side file write. Env var list ended up shorter
      than even planned: no log file exists to tail (this process logs to stdout, not a file), so
      `summary` and the full report are identical here — only the desktop version has a log to trim
      down to a shorter summary.

## 12. Frontend wiring

- [x] 12.1 In `ui/src/lib/api.web.ts`, replace the auth stubs (`getAccount`, `getAccountIdentities`,
      `switchAccount`, `signOut`, `getGoogleAccounts`, `switchGoogleAccount`, `removeGoogleAccount`)
      and `loginWebview` with a cookie-paste equivalent (a new exported function or a repurposed
      call site — the existing cookie-paste UI, if any already exists desktop-side, is the
      reference for what fields it needs) calling section 2's routes. Verify: `pnpm check` passes.
      — Added `signInWithCookie` (posts to `/api/auth/cookie`); `loginWebview` stays `unsupported`
      with an updated message (no webview to host it in a browser, cookie-paste is the browser
      login path per design.md). Found missing routes for `getGoogleAccounts`/
      `switchGoogleAccount`/`removeGoogleAccount` during this task (ported in 1.4, never exposed in
      2.4) — added them to `crates/server/src/auth.rs`. `pnpm check`: 0 errors.
- [x] 12.2 Add a `ui/src/lib/ws.ts` client connecting to `/ws`, dispatching frames by `type` to the
      same callback registries `onX(...)` functions in `api.web.ts` already expose the shape of.
      Replace every `noSubscription()`-stubbed `onX` with a real subscription through this client.
      Verify: `pnpm check` passes; a manual connect while signed in receives at least one real
      event (e.g. `onAuthChanged` firing after 12.1's sign-in). — `ws.ts` added (lazy-connect,
      per-event-type listener sets, auto-reconnect with a 2s delay while someone's still
      listening). `onAuthChanged`/`onAccountSelectionRequired`/`onLastfmState` wired through it.
      Live event delivery not separately re-verified this session — no real YouTube cookie
      available here to drive a real sign-in (same gap as 2.2/8.1).
- [x] 12.3 Replace the library/history/play-counts stubs (`getLibrary`, `getLibraryAlbums`,
      `getLibraryArtists`, `getUploadAlbums`, `getHistory`, `getPlayCounts`, `getLocalPlaylists` —
      the last one stays unsupported, it's desktop-local-SQLite-specific, see design.md) with real
      calls against section 3's routes. Verify: `pnpm check` passes.
- [x] 12.4 Replace the playlist-write stubs (`createPlaylist`, `addToPlaylist`,
      `removeFromPlaylist`, `removeManyFromPlaylist`, `editPlaylistDetails`, `setPlaylistCover`,
      `deletePlaylist`, `setPlaylistSort`) with real calls against section 4's routes. Verify:
      `pnpm check` passes. — `createPlaylist`'s `local: true` path stays `unsupported` (no
      server-side local playlists, see design.md); `setPlaylistCover` takes a `Blob`/object URL on
      the web build rather than a filesystem path.
- [x] 12.5 Replace the ratings stubs (`rate`, `setSongSaved`, `setAlbumSaved`, `subscribe`) with
      real calls against section 5's routes. Verify: `pnpm check` passes.
- [x] 12.6 Replace the settings stubs (`getSettings`, `setSetting`, `getStreamClients`,
      `clearCaches`) with real calls against section 6's routes; leave `setAppIcon`, `appIconPath`,
      `allowFontFile` as permanent `unsupported(...)` stubs with an updated comment explaining why
      (desktop-only, no browser equivalent — see design.md/master plan's scope cuts). Verify:
      `pnpm check` passes. — `getSettings` is a bulk call on desktop; found no bulk route existed
      (only per-key), so added `get_all_settings()`/`GET /api/settings` (task 6's section updated
      to match).
- [x] 12.7 Replace the lyrics stubs (`getLyrics`, `chooseLyricsSource`, `setLyricsOffset`,
      `lyricsProviders`) with real calls against section 7's routes. Verify: `pnpm check` passes.
- [x] 12.8 Replace the Last.fm stubs (`lastfmStatus`, `lastfmConnect`, `lastfmDisconnect`) with
      real calls against section 8's routes. Verify: `pnpm check` passes. — `lastfmConnect` opens
      the authorize URL the server now returns (task 1.5's adaptation) in a new tab/window instead
      of an in-app browser-open call.
- [x] 12.9 Replace the blocked-artists stubs (`getBlockedArtists`, `blockArtist`, `unblockArtist`)
      with real calls against section 9's routes. Verify: `pnpm check` passes.
- [x] 12.10 Add a `ui/src/lib/lt-client.ts` that opens its own `WebSocket` directly to the relay URL
      `ltSetServerUrl` stores (not `ws.ts`/`/ws` from 12.2 — a separate connection to a separate
      process), reimplementing `listen_protocol`'s `ClientMessage`/`ServerMessage` enums as
      TypeScript types (same JSON shape `tokio-tungstenite` already sends/expects — no Rust changes
      needed). Replace the Listen Together stubs (`ltGetState`, `ltCreateRoom`, `ltJoinRoom`,
      `ltLeave`, `ltApproveJoin`, `ltRejectJoin`, `ltKick`, `ltTransferHost`, `ltApproveSuggestion`,
      `ltRejectSuggestion`, `ltRequestSync`) with real calls through it; `ltSetServerUrl` itself
      just needs to actually store the URL (it may already — check before assuming it's a stub).
      Verify: `pnpm check` passes; with `cargo run -p sync-server` running locally, two browser tabs
      — one creates a room (gets a code), the other joins (pending), the host approves it, both see
      the updated participant list; host kicks a participant (that connection is notified and
      closed); a non-host participant leaves voluntarily (others notified); with host A and
      participant B connected, disconnect both without a transfer message, then reconnect B alone —
      confirm B is not treated as the new host (the existing desktop fix
      `specs/http-listen-together/spec.md` references). — `lt-client.ts` is a from-scratch TS port
      of `src-tauri/src/listentogether/mod.rs`'s connection/state machine (room state mirror, join
      approval flow, host transfer, reconnect-with-token-and-backoff), minus the `SyncCommand`/
      bridge-to-player machinery (no local playback on this server yet — Phase 3). `ltSetServerUrl`
      was in fact still a stub; it now just sets a field and notifies subscribers, same as the
      desktop command. **Fully confirmed live**: built and ran `crates/sync-server` locally
      (`cargo run -p sync-server`, `PORT=8799`), drove it with two real WebSocket clients
      reproducing `lt-client.ts`'s exact message shapes — create_room → room_created, join_room →
      join_request/approve_join → join_approved/user_joined, transfer_host → host_changed (both
      sides), kick_user → kicked/user_left — all field names and nesting matched the TS types
      exactly (`state.users[].user_id`, etc.), no mismatches found. The disconnect-without-transfer
      reconnect scenario is server-side-only logic (already covered by
      `specs/http-listen-together/spec.md` against the existing `crates/sync-server`, unchanged by
      this task) — not re-tested here since no client code governs it. `pnpm check`: 0 errors.
- [x] 12.11 Replace the diagnostics stubs (`diagnostics`, `diagnosticsSummary`, `saveDiagnostics`)
      with real calls against section 11's routes, `saveDiagnostics` triggering a browser download
      instead of taking a path argument (adjust its call sites accordingly). Verify: `pnpm check`
      passes. — `saveDiagnostics` uses `window.open` on the server's `/api/diagnostics/save`
      (`Content-Disposition: attachment` already set server-side) rather than a save-path argument.

## 13. UI chrome

- [x] 13.1 In `ui/src/routes/+layout.svelte`, extend the non-Tauri branch to render
      `Sidebar`/a lightweight nav instead of the current minimal shell, now that the data it needs
      (library, account state) is real. Do not render a `PlayerBar` with working controls yet — no
      playback exists (Non-Goal) — either omit it or render it visibly disabled/inert. Verify:
      `pnpm build && pnpm preview`, loaded in a plain browser while signed in, shows real nav and
      library content; no console error from a component expecting Tauri-only APIs. — `PlayerBar`
      omitted entirely (matches the Non-Goal — nothing to show yet). Added `initWebApp()` to
      `player.svelte.ts`: `initApp`'s account/library/blocked-artists/Listen-Together wiring, minus
      everything playback-only (no queue/now-playing subscriptions, no `scanLocal`/prefs-from-
      settings, no locale push to Rust). Also added the toast banner (components already call
      `toast(...)`, e.g. block/unblock, and had nowhere to show it outside the Tauri branch).
      **Added scope found, not in the spec**: Sidebar's existing Settings button sets
      `ui.settingsOpen = true`, but `SettingsDialog` isn't mounted in the web branch — it imports
      `@tauri-apps/plugin-dialog`'s `open`/`save` and `@tauri-apps/api/app`'s `getVersion`
      unconditionally, which 13.2's audit (below) doesn't cover and this task didn't ask for
      auditing/fixing (it's 1511 lines, mostly desktop-only settings — autostart, mpv
      crossfade, Discord RPC, ambient WebGL, proxy, window decorations — a real follow-up change of
      its own, not a quick fix). Resolved the dead-button symptom without that larger audit: Sidebar
      now hides the Settings button entirely on `!isTauri` rather than show one that silently does
      nothing — honest about what's missing instead of a button that looks broken. — **Verified
      live**: built `crates/server` and ran plain `vite dev` (not `--mode tauri`) together, loaded
      in headless Chrome
      (`--dump-dom` + a CDP console listener against the real page, not a one-off test harness) —
      `<aside>`/nav links (`/`, `/search`, `/library`)/the "Limusic" brand all render, zero console
      errors or exceptions (one pre-existing, unrelated Svelte dev warning at
      `player.svelte.ts:414` survived the check — not introduced by this task).
- [x] 13.2 Confirm `ui/src/lib/components/Sidebar.svelte` doesn't call any Tauri-only API
      unconditionally (same class of bug `offline-local-music-pwa`'s "Added scope" note found for
      `+layout.svelte`/`Titlebar`/`ResizeBorders`). Verify: the sidebar renders and its nav links
      work in the same browser session as 13.1, with no uncaught exception. — Confirmed clean: no
      `@tauri-apps` import, no `invoke`, no `getCurrentWindow` anywhere in the file. Updated its one
      stale comment, which still claimed Sidebar was Tauri-only and pointed at the now-deleted
      `+page.ts` redirect (removed in `http-browse-vertical-slice`). Verified live together with
      13.1 above — same headless-Chrome pass covers both.

## 14. Verification

- [~] 14.1 End-to-end, signed in, plain browser tab (no Tauri): cookie-paste sign-in, browse
      library, view and edit a playlist, rate a song, save an album, check Last.fm status, fetch
      lyrics for a playing-in-another-tab-irrelevant track (no playback yet, so pick any track id),
      block then unblock an artist, open a Listen Together room from a second browser tab and
      approve its join. Verify: every step works against real data with no uncaught frontend error.
      — **Partially verified, honestly incomplete**: this environment has no real YouTube session
      cookie (same gap documented throughout this change — 2.2, 8.1, 12.2), so the actual
      cookie-paste sign-in and everything gated behind it (library/playlist-edit/rate/save/
      Last.fm-status) could not be driven end-to-end here; that needs a human with a real account.
      What *was* verified live this session, standing in for the pieces that don't need a session:
      the web shell itself (Sidebar/nav, no console errors — 13.1/13.2), every route's session
      gating (14.2 below), lyrics fetch with real synced lyrics (7.1), blocked-artist block/unblock
      round-trip (9.1), and Listen Together's full wire protocol — create/join/approve, host
      transfer, kick — against a live `crates/sync-server` with two real WebSocket clients (12.10).
      Not re-verified through the actual browser UI: a human still needs to click through the
      signed-in paths at least once with a real account, same as the desktop app's own 2.2 gap.
- [x] 14.2 Confirm anonymous (signed-out) behavior is unchanged from `http-browse-vertical-slice`:
      search/browse still work with no session; every signed-in-only route returns the clear
      client error from `http-auth`'s spec, not a crash or wrong-account data. — **Verified live**:
      `/api/search`, `/api/search/all`, `/api/blocked-artists`, `/api/lyrics/providers` all `200`
      with no session; `/api/library`, `/api/library/albums`, `/api/history`,
      `/api/auth/identities`, `/api/settings`, `/api/lastfm/status` all a clean `401` with no
      session (not a crash). `/api/browse/home` returned a `502` (YouTube's own `browse` endpoint
      answered `400` to this environment's anonymous request) — pre-existing/environment-specific,
      not a regression: that route belongs to `http-browse-vertical-slice`, nothing it depends on
      was touched by this change, and the failure is an upstream HTTP error surfaced as-is, not a
      server crash or wrong-account data (which is what this task actually checks for).
- [x] 14.3 Confirm the existing Tauri desktop build still works unchanged: run `cargo tauri dev`,
      sign in, use library/playlists/ratings/Last.fm/lyrics/blocked-artists/Listen Together there.
      Verify: all work exactly as before, via `invoke`/Tauri events, with `crates/server`/
      `crates/core` never involved — `src-tauri` was never touched (section 1.7's constraint held).
      — Confirmed `src-tauri`'s only diff on this branch, across the *entire* change, is
      `tauri.conf.json`'s `beforeDevCommand`/`beforeBuildCommand` (the dev/build-separation fix from
      earlier in this session, unrelated to this change's scope) — zero Rust source touched.
      `cargo check -p limusic-app` compiles clean. A full interactive `cargo tauri dev` click-through
      with a real account is the same class of human-needed gap as 2.2/14.1 above — not attempted
      here, but the no-source-touched guarantee is what this task is really insuring against, and
      that's directly confirmed.
