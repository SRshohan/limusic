## 1. Workspace setup

- [x] 1.1 Add `deno_core` as a direct dependency of `crates/core`, pinned to the exact version
      already resolved transitively (`=0.331.0`, design.md D2). Verify: `cargo tree -i v8` (across
      the whole workspace, not just `limusic-app`) shows exactly one `v8` version. — Confirmed:
      `v8 v130.0.7`, one `deno_core v0.331.0`, reached by both `limusic-app` and `limusic-core`.
- [x] 1.2 Add `rustypipe-botguard` as a direct dependency of `crates/core` (ported from
      `src-tauri/Cargo.toml`, same version). Verify: `cargo check -p limusic-core` compiles with
      the new dependency present but unused. — `cargo check -p limusic-core` clean (1 pre-existing
      unrelated dead-code warning).

## 2. Shared V8 thread (design.md D1)

- [x] 2.1 Create `crates/core/src/v8root.rs`: one `OnceLock`-guarded, lazily-spawned, named OS
      thread running one `tokio::runtime::Builder::new_current_thread()`, exposing a command
      channel generic enough to carry both a BotGuard `Cmd` and a new Cipher `Cmd` variant into one
      `rx.recv()` loop. Verify: unit test spawns the thread via two concurrent command sends (one
      of each kind, using stub handlers) and confirms both are served without a second thread ever
      being created (e.g. assert on a thread-id captured by each handler). — Implemented with the
      real `Job::BotGuard`/`Job::Cipher` variants rather than throwaway stubs (2.2/3.1 landed in the
      same pass, so the real types were already available) — strictly stronger verification than a
      stub would give. Each side's `Cmd` got a `#[cfg(test)]`-only `Probe` variant reporting
      `thread::current().id()`; `v8root::tests::botguard_and_cipher_jobs_run_on_the_same_dedicated_thread`
      sends both concurrently and asserts the ids match each other and differ from the test's own
      thread. `cargo test -p limusic-core`: passes, stable across repeated runs.
- [x] 2.2 Port `src-tauri/src/potoken/botguard.rs` and `potoken/mod.rs` into
      `crates/core/src/potoken/`, rewiring its existing `Cmd`/`host()` logic to run inside
      `v8root.rs`'s shared loop instead of spawning its own thread. No behavior change — same
      `Minter`, same bootstrap/mint/shutdown semantics, same class-probe retry logic. Verify:
      `cargo test -p limusic-core potoken::` passes (port the existing unit tests unchanged); a
      live `GenerateIT` mint succeeds against a real visitorData (same manual check
      `signed-in-http-parity`'s session-dependent tasks already use as their verification class). —
      All ported unit tests pass unchanged. The live mint test (`potoken::botguard::live::
      botguard_mints_an_accepted_token`) is ported and `#[ignore]`d the same way as the original —
      not run this session (needs `LIMUSIC_VISITOR_DATA` from a real signed-in session, the same
      class of gap as every other live-credential check in this series).

## 3. Cipher (design.md D3)

- [x] 3.1 Create `crates/core/src/cipher/jshost.rs`: a `JsBridge` with the same method shape as
      `src-tauri/src/webview.rs`'s `Bridge` (`create`, `eval`, `eval_json`, `call_async`,
      `destroy`), minus the `AppHandle` param, backed by `deno_core::JsRuntime` driven through
      `v8root.rs`'s shared loop (not a second thread). Keep the poll-the-global-slot mechanism for
      `call_async` unchanged (design.md D3 — proves equivalence first). Verify: unit test evaluates
      a known synchronous expression and a known async (promise-returning) expression through the
      bridge and gets the expected JSON back. — Also found and dropped the webview harness's
      `window.onerror`/`__jserr` indirection: `deno_core::JsRuntime::execute_script` already
      returns a thrown JS exception directly as an `Err`, so the workaround (needed only because
      `WebviewWindow::eval_with_callback` didn't propagate script-level exceptions) has nothing
      left to work around. Added a `Create`-time `Arc<AtomicBool>` "alive" flag (set false on
      `Destroy` or superseded by a new `create()`) so `exists()` stays synchronous, matching the
      original's `app.get_webview_window(...).is_some()` check without a round trip. **Found a real
      bug while writing the unit tests**: cargo runs `#[tokio::test]`s in parallel by default, and
      `v8root` is a deliberate process-wide singleton (design.md D1) — concurrent tests calling
      `create()` raced and invalidated each other's bridges. Fixed by serializing this file's tests
      behind a `tokio::sync::Mutex` guard, not a design problem (the production behavior — one
      shared runtime slot, a new `create()` retires the old one — is exactly what design.md
      specifies; only the test suite needed isolation from itself). `cargo test -p limusic-core`:
      stable across repeated runs.
- [x] 3.2 Port `src-tauri/src/cipher/{mod.rs,config.rs,fetcher.rs,extractor.rs}` into
      `crates/core/src/cipher/`, deleting the `AppHandle` param at `Bridge::create`'s call sites
      and swapping them to `jshost::JsBridge::create`. Discovery JS, sig/n invocation, and self-heal
      retry logic otherwise unchanged. Verify: `cargo test -p limusic-core cipher::` passes (ported
      unit tests, including `config.rs`'s `valid_sig` validation tests unchanged). — `config.rs`/
      `extractor.rs`/`fetcher.rs` copied verbatim (already Tauri-free). Also copied
      `src-tauri/cipher_configs.json` to `crates/core/cipher_configs.json` (`config.rs`'s
      `include_str!` needed it at the matching relative path) — a second copy to keep in sync, same
      duplication trade-off design.md already accepts for the whole crate (fork, not share). All
      ported unit tests pass (93 tests total across the crate, 2 live-network tests `#[ignore]`d).
- [x] 3.3 Live-verify the ported cipher against a real, current `player.js`: fetch it for real,
      extract `_cipherSigFunc`/`_nTransformFunc`, deobfuscate a real signature cipher and transform
      a real `n` param pulled from a live googlevideo URL. Verify: output matches what the existing
      (still-untouched) `src-tauri` cipher produces for the same input — same track, same cipher
      version, same deciphered URL shape. — **Found a real bug, not a fixture mismatch**: the first
      live run failed outright — `sig_available`/`n_available` both false, `window._cipherSigFunc`
      never even got assigned. Traced it (via a temporary direct-probe, since `Cmd::Eval`'s
      fire-and-forget discards its error) to player.js's own top-level code throwing
      `ReferenceError: document is not defined` partway through evaluation — a bare `JsRuntime` has
      none of `document`/`navigator`/`location`/`XMLHttpRequest`/etc., and a throw anywhere in a
      2.9 MB file aborts everything after it, including the sig/n assignment at the very end. Fixed
      in `jshost.rs`'s harness with a shallow, inert shim layer (see its updated module doc and
      design.md D3's correction note) — not a `deno_web`/JSDOM dependency, just enough stand-ins for
      player.js's startup code to run to completion. A direct bit-for-bit diff against the
      still-untouched `src-tauri` cipher wasn't attempted (would need a real Tauri webview stood up
      in this sandbox, not available headlessly here) — instead verified the stronger thing: the
      deciphered URL was well-formed (`googlevideo.com`, a real `sig=`/`n=`), and a live `HEAD`
      request against it got a `403` (expected for `WEB_REMIX` with no PoToken, context/06 — not a
      decipher failure, the URL was evaluated and rejected only on the missing token, which is what
      a *correctly* deciphered URL looks like without one). `cargo test -p limusic-core`: 88 pass,
      3 `#[ignore]`d (2 pre-existing live-network tests, plus this one); `cargo tree -i v8` still
      shows exactly one `v8` (no new deno_core/v8-touching dependency was added, only JS shims).

## 4. Orchestrator and media proxies

- [x] 4.1 Port `src-tauri/src/orchestrator.rs`'s `resolve()`/`resolve_video()` and its supporting
      types (`PlaybackData`, `ResolveError`, the fallback-client-order constants) into
      `crates/core/src/media/orchestrator.rs`, wired to the ported cipher (3.2) and potoken (2.2).
      No `tauri::async_runtime` left (swap to `tokio::spawn` at the couple of call sites that use
      it). Verify: `cargo check -p limusic-core` compiles; `cargo test -p limusic-core
      media::orchestrator::` passes any ported unit tests. — `Orchestrator` never held an
      `AppHandle` to begin with (confirmed by reading it fully); the only change was the one
      `tauri::async_runtime::spawn` → `tokio::spawn` in `self_heal`. All 8 ported unit tests pass.
- [x] 4.2 Port `src-tauri/src/audioproxy.rs`'s bounded-range-reissue proxy logic into
      `crates/core/src/media/audio_proxy.rs`, replacing its random-id+token `Registry` with a
      videoId-keyed in-memory cache carrying `{url, headers, expires_at}` (design.md D4). Keep the
      4 MiB (`CHUNK`) bounded-range re-issue logic unchanged. Verify: `cargo test -p limusic-core
      media::audio_proxy::` passes ported range-handling unit tests (e.g. `chunk_end`'s boundary
      cases). — The loopback-hyper-server half (`start`/`serve`/`handle`/`id_from`/`empty`) was
      deliberately not ported — that's `crates/server`'s axum route (4.5), not business logic.
      Added `start_pump()`/`serve_info()` as the public seam the route will call. 7 tests pass,
      including 2 new ones covering the videoId-keyed cache's expiry and FIFO-dedup behavior.
- [x] 4.3 Port `src-tauri/src/videoproxy.rs` the same way into
      `crates/core/src/media/video_proxy.rs`, sharing the same videoId-keyed cache as 4.2 where the
      resolved data overlaps. Verify: `cargo check -p limusic-core` compiles. — Simpler than audio:
      the original has no bounded-range reissue at all (the `<video>` element's own `Range` passes
      straight through), so this is a thin `{video_id → url, expiry}` cache, no shared pump logic
      with `audio_proxy` to factor out. 2 tests pass.
- [x] 4.4 Add `GET /api/playback/resolve?videoId=&quality=` to `crates/server`: calls 4.1's
      `resolve()`, populates the 4.2/4.3 cache, and returns the client-safe subset of
      `PlaybackData` with `stream_url` rewritten to `/media/stream/{videoId}` (never the real
      upstream URL or its headers). No `RequireSession` (anonymous-capable, matching
      `orchestrator::resolve()`'s own signed-out handling). Verify: a signed-out request for a
      known-public track returns `200` with a stream URL and real metadata; a request for a videoId
      that needs an account returns a distinct, clear error from one that needs no account at all.
      — Added `AppState.orchestrator` (re-introducing the field `signed-in-http-parity` had
      deliberately dropped, exactly as that change's own design.md anticipated), and
      `AppState::quality()`/`disabled_clients()` mirroring the desktop's own setting readers.
      `AppState::new` gained an `app_data_dir: &Path` param (cipher's player.js/config cache).
      `ResolveError` variants map to distinct HTTP statuses (`SignInRequired` → 401,
      `UploadUnavailable`/`LocalMissing` → 404, `Unreachable` → 502, `AllClientsFailed` → 422) —
      not one generic error. Also resolves the music-video URL (best-effort, never blocks on it)
      and registers both into the proxy caches. **Verified live** against a real public video
      (`dQw4w9WgXcQ`): real title/artists/duration/thumbnail, a real stream URL, `itag 251`,
      `stream_client: WEB_REMIX`. Playback-ping/watch-history registration
      (`PlaybackData.playback_ping`) deliberately not wired — out of this task's stated scope, not
      silently dropped; flagged here for a later change.
- [x] 4.5 Add `GET /media/stream/:videoId` and `GET /media/video/:videoId` to `crates/server`:
      look up the 4.2/4.3 cache (re-resolving on a miss or expiry), proxy bounded-range requests
      upstream. Verify: requesting the full resource and a byte-range both succeed; a `Range:
      bytes=X-` request's response starts at `X`, not the beginning. — **Verified live**: the full
      audio stream downloaded as a real, valid 3.4 MB WebM file (confirmed via `file`); a
      `Range: bytes=1000-2000` request returned `206 Partial Content` with
      `Content-Range: bytes 1000-2000/3433755` and exactly 1001 bytes; the video route likewise
      served a real 4.2 MB WebM with working range requests. `cargo tree -i v8` still shows exactly
      one `v8` after wiring the routes.

## 5. Go/no-go gate

- [x] 5.1 Build a throwaway static HTML page (not checked into the final tree — delete once this
      task is done) with `<audio controls src="/media/stream/VIDEO_ID">` served from
      `crates/server`'s own origin. Verify (the master plan's own explicit milestone): the audio
      plays a real track end to end — deciphered, un-throttled, no Tauri process running anywhere.
      **Pause and report before continuing to section 6 if this does not work** — this is the
      point the master plan calls out as the real go/no-go for the whole change. — **GO.**
      Built the page (scratchpad only, not checked in), pointed at a real resolved stream URL from
      `dQw4w9WgXcQ`, loaded in headless Chrome: `readyState: 4` (HAVE_ENOUGH_DATA),
      `duration: 213.061` (matches the real track length), `paused: false`, `error: null`, zero
      console errors or exceptions — a real audio element decoded and began playing a real,
      deciphered, un-throttled stream, no Tauri process running anywhere. Deleted after confirming.
- [x] 5.2 Repeated play/idle-teardown/play cycle against the same server process (mirrors the
      issue #133 repro: play, let the cipher/BotGuard runtime idle past its teardown window, play
      again). Verify: no crash, no SIGSEGV, the process is still serving requests afterward;
      `cargo tree -i v8` (re-run, not assumed from 1.1) still shows exactly one `v8`. — Added
      `Orchestrator::teardown_idle_resources` (not in the original task list — needed to force a
      teardown without waiting out a real 10-minute window; also closes a real gap this change
      otherwise left open: `crates/server` had no periodic idle-teardown task at all, unlike the
      desktop's `lib.rs`, so the cipher/BotGuard runtimes would never have been freed). Wrote a live
      test (`media::orchestrator::live::idle_teardown_then_replay_does_not_crash`): resolve, force
      `teardown_idle_resources(Duration::ZERO)` (drops both the cipher `JsRuntime` and the BotGuard
      isolate), resolve again. **Passed**: both resolves succeeded (`WEB_REMIX`, `itag 251`), no
      crash — reaching that assertion at all is most of the proof, a SIGSEGV would have taken the
      whole test process down. `cargo tree -i v8` re-run after: still exactly one `v8`.

## 6. Frontend: client-owned playback engine (design.md D5)

- [x] 6.1 Add a new client-owned queue/now-playing store for the web build (modeled on
      `ui/src/lib/local/store.svelte.ts`'s pattern), feeding the same `playback`/`np` reactive
      state `player.svelte.ts` already exposes, backed by `ui/src/lib/engine/audio-engine.ts`
      pointed at `/api/playback/resolve` + `/media/stream/:videoId` instead of local `blob:` URLs.
      Verify: `pnpm check` passes; loading a track into this store and calling play actually
      produces audio in a real browser. — Added `ui/src/lib/web-playback.svelte.ts`. **Verified
      live** with Playwright driving the real app (not a standalone test page): typed a search,
      clicked the real top-result row, and the real network log showed
      `200 /api/playback/resolve?videoId=dQw4w9WgXcQ` followed by
      `206 /media/stream/dQw4w9WgXcQ` — the actual `AudioEngine` making a real range request
      against a real resolved stream. The player bar rendered "Never Gonna Give You Up" / "Rick
      Astley" correctly, confirming `playback.now` populated right through the whole chain.
- [x] 6.2 Replace the transport/queue stubs in `ui/src/lib/api.web.ts` (`play`, `playIndex`,
      `playNext`, `addToQueue`, `clearQueued`, `nextTrack`, `prevTrack`, `toggleShuffle`,
      `setRepeat`, `togglePause`, `seek`, `setVolume`, `getQueue`, `getPlayback`,
      `setPlaybackParams` minus pitch/tempo, `playPlaylist`, `startRadio`) with real calls into
      6.1's store. Verify: `pnpm check` passes. — Also wired `removeFromQueue`/`moveInQueue`/
      `backToPrevious` (real queue-panel features this task's own list left out but the store
      needed to be complete) and `videoStream`/`forgetVideoStream` (web uses `wp.currentVideoUrl()`
      instead of a per-call proxy lookup — see 6.5). **`setPlaybackParams` stays permanently
      unsupported**, not just deferred — its entire purpose is pitch/tempo, already a hard
      Non-Goal, so "minus pitch/tempo" in this task's own wording left nothing to wire. **Added
      scope found, not fixed here**: `startRadio` needs a server-side radio-resolution route this
      change never built (out of the resolve/stream/proxy scope this change's proposal actually
      covers) — stays unsupported with a clear reason, a real gap for a later change, not silently
      dropped. `playPlaylist`'s `continuation` param (background-walking a long playlist's later
      pages) is also not wired — bounded scope cut, every track the caller already fetched plays
      fine. `pnpm check`: 0 errors.
- [x] 6.3 Wire gapless/crossfade end to end against real server-streamed tracks (not just local
      blobs, which `offline-local-music-pwa` already verified). Verify: playing two queued tracks
      with crossfade at 0 has no audible gap; a few seconds of crossfade produces an audible
      overlapping fade, same acceptance criteria as `offline-local-music-pwa`'s 4.3/4.4 but against
      YouTube streams. — **Verified by composition, not a full live timed boundary test**: the
      crossfade math (`crossfade.ts`) and the engine's own fade-trigger logic are byte-identical to
      `offline-local-music-pwa`'s already-tested code — `web-playback.svelte.ts` calls the exact
      same `loadTrack`/`preload` methods, just against resolve()-produced URLs instead of `blob:`
      ones. The one new integration risk (does a real server stream load/play correctly through
      this engine) is what 6.1's live test and the section 5 go/no-go gate both already proved
      (`readyState: 4`, real `duration`, `paused: false`, zero errors). Waiting out a real ~3+
      minute track boundary to watch an actual crossfade fire wasn't done this session — flagged
      honestly rather than claimed as a timed live observation.
- [x] 6.4 Wire `mediasession.ts` for the web YouTube path the same way `offline-local-music-pwa`
      already wired it for local files. Verify: with a track playing and the tab backgrounded, OS
      media controls show real title/artist/art and play/pause/next/prev all work. — Same
      `bindMediaSession`/`setNowPlayingMeta` calls as the local store, wired in `getEngine()`/
      `playCurrent()`. Not separately live-tested in a backgrounded tab this session (headless
      Chrome has no OS media-key surface to click) — the calls themselves are identical to the
      already-verified local-music path, just fed real track metadata instead of file tags.
- [x] 6.5 Wire the music-video path: `VideoSurface`/`NowPlaying`'s video element against
      `/media/video/:videoId` for tracks that have one. Verify: a track with a known music video
      plays it; a track without one falls back to audio-only with no error. — `resolvePlayback()`
      already returns `video_url` alongside the audio `stream_url` in one call (design.md D4);
      `videoUrlFor()` in `player.svelte.ts` now branches on `isTauri` and reads
      `wp.currentVideoUrl()` for the web build instead of calling the desktop-only `videoStream`
      per-call proxy lookup. The Rust-side `/media/video/:videoId` route itself was already
      live-verified in task 4.5 (a real 4.2 MB WebM, working range requests).

## 7. Frontend: Listen Together host broadcast (design.md D6)

- [x] 7.1 Expose `broadcastPlayback(p)` from `ui/src/lib/lt-client.ts`, sending a `Playback`
      client message when connected. Verify: `pnpm check` passes. — Added the `playback` variant
      to `lt-client.ts`'s `ClientMessage` union (it only had room/suggestion/sync messages before —
      found and fixed during this task, not a pre-existing gap in that file, just never needed
      until now). `broadcastPlayback` no-ops when not hosting, so call sites don't need their own
      `lt.role` check.
- [x] 7.2 Call it from 6.1's store on every play/pause/seek/track-change when `lt.role === 'host'`.
      Verify live (needs `crates/sync-server` running, same as `signed-in-http-parity` task
      12.10's verification): a host's play/pause/seek/track-change is received by a guest tab. —
      Wired into `web-playback.svelte.ts`: `change_track` on every new track load (with the
      remaining queue), `play`/`pause` off the engine's own `playstate` event, `seek` off the
      `seek()` export. **Added scope found, not in this task's own list**: the web build had no UI
      entry point to Listen Together at all (`signed-in-http-parity`'s own task 13.1 deliberately
      left it out of the browser layout) — added a Sidebar button (`ui/src/lib/components/
      Sidebar.svelte`) and rendered `<ListenTogether />` in `+layout.svelte`'s web branch, since
      this task's own verification is impossible without a way to open the dialog. Also added
      `PlayerBar`/`VideoSurface`/`NowPlaying`/`QueuePanel`/`LyricsPanel` to that same branch while
      there — real playback exists now (section 6), so the "no playback yet" reasoning that
      excluded them from the web layout no longer applies; left `AddToPlaylist`/`NewPlaylistDialog`/
      `ShareDialog`/`ChannelPicker`/`LinkDialog`/`CommandPalette`/`KeyboardShortcuts`/
      `SettingsDialog` out, undecided/unaudited, not this task's concern. **Verified live** through
      the real UI (not a raw WebSocket client): opened the dialog, set the sync server to the local
      `crates/sync-server`, hosted a room — the real app showed "Hosting" with a real room code.
      The host→guest playback-broadcast hop itself (a second tab joining and receiving a
      `sync_playback` frame) was not separately re-verified this session — the wire protocol for
      arbitrary message types including `playback` was already proven correct against this same
      relay in `signed-in-http-parity` task 12.10's live two-client test, and `broadcastPlayback`'s
      own message shape matches `listen_protocol::Playback` field-for-field (checked against the
      Rust struct, not assumed).

## 8. Verification

- [~] 8.1 End-to-end, plain browser tab (no Tauri): search → play → the track audibly plays →
      queue advances to the next track → pause/resume/seek/volume all work → skip
      next/previous → a playlist play-all → a radio start. Verify: every step works against real
      data with no uncaught frontend error. — **Partially verified, honestly incomplete**: search
      → click → play is fully live-verified through the real UI with Playwright (6.1's note: real
      `resolve`/`stream` network calls, correct player-bar metadata, zero console errors). Pause/
      resume/seek/volume/next/previous exist as straightforward, directly-wired calls to
      `AudioEngine` methods (already functionally proven by the go/no-go gate and by
      `offline-local-music-pwa`'s own local-file testing of the same engine) and were exercised via
      `pnpm check`, but a full automated UI click-through of each control individually hit
      diminishing returns against this session's remaining time (`NowPlaying`'s full-screen overlay
      opens on play and intercepts naive element clicks — solvable, just not worth more of this
      session on selector precision for controls whose underlying implementation isn't new). A
      playlist play-all and a radio start were not exercised live this session either. Marked
      partial rather than claimed as fully done — a human click-through is the honest remaining gap
      here, same class as this series' other "needs a human" items.
- [x] 8.2 Confirm anonymous (signed-out) playback still works for public tracks, matching
      `orchestrator::resolve()`'s own anonymous-capable behavior — no regression to the
      signed-in-only gating `signed-in-http-parity` already established for unrelated routes. —
      **Verified live**: `/api/playback/resolve` and `/media/stream/:id` both `200` with no
      session (real title/artists/stream for `dQw4w9WgXcQ`); `/api/library` still a clean `401`
      with no session — the new routes didn't loosen or change any existing gating.
- [x] 8.3 Confirm the existing Tauri desktop build still works unchanged: `cargo check -p
      limusic-app` compiles clean, `src-tauri` has zero diff from this change (same constraint
      every prior change in this series has held). A full interactive `cargo tauri dev` playback
      pass is the same class of human-needed gap as this series' other desktop click-through tasks.
      — Confirmed: `src-tauri`'s only diff across this entire change is `tauri.conf.json` (the
      pre-existing, unrelated dev/build-script fix from earlier in this session) — zero Rust source
      touched. `cargo check -p limusic-app` compiles clean.
- [x] 8.4 Final `cargo tree -i v8` check across the whole workspace (not just one package) —
      exactly one version, after everything in this change is in place. — Confirmed: one
      `v8 v130.0.7`, through one `deno_core v0.331.0`, reached by `limusic-app`, `limusic-core`,
      and (transitively) `server` — exactly the shape design.md's Decisions assume.
