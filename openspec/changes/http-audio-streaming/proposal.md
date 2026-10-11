## Why

The browser/PWA build has full signed-in HTTP parity for everything except the one thing a music
player is actually for: playing a YouTube track. Every transport/queue function in `api.web.ts`
(`play`, `playIndex`, `togglePause`, `seek`, `getQueue`, `getPlayback`, …) is still an
`unsupported()` stub — explicitly deferred by `signed-in-http-parity`'s own Non-Goals. This is the
master conversion plan's Phase 3 ("Streaming", called out there as the highest-risk phase) plus the
playback-wiring slice of Phase 4: without it, the browser app can browse and edit everything but
never actually listen to it.

## What Changes

- Port the desktop's YouTube stream-resolution pipeline — signature-cipher execution
  (`src-tauri/src/cipher/`, `src-tauri/src/webview.rs`), PoToken/BotGuard (`src-tauri/src/potoken/`,
  already Tauri-free), and `orchestrator.rs`'s `resolve()` — into `crates/core`, replacing the
  hidden-webview cipher host with a headless `deno_core`-backed one. BotGuard already runs
  headlessly today; only the cipher needs new work.
- Add a `crates/core/src/v8root.rs` solving the V8-singleton-thread constraint: every thread that
  touches V8 must descend from whichever thread first called `v8::Initialize`, or the process
  SIGSEGVs (this already happened once for BotGuard alone, issue #133; adding a second V8 consumer
  makes it sharper, not easier).
- Fold `audioproxy.rs`/`videoproxy.rs` (already plain hyper-based loopback range-proxies solving
  googlevideo's throttling of open-ended ranges, not mpv-specific) into `crates/server` axum
  routes: `GET /media/stream/:videoId`, `GET /media/video/:videoId`.
- Add `GET /api/playback/resolve?videoId=&quality=` returning the non-proprietary parts of
  `PlaybackData` (title/artists/duration/thumbnail/a stream URL rewritten to the proxy route) —
  anonymous-capable, same as search/browse (`orchestrator::resolve()` already works signed-out).
- Wire real playback into the frontend: `player.svelte.ts`'s queue/transport state becomes
  **client-owned** on the web build (there is no server-side `AppState.player`/orchestrator pushing
  events anymore — the browser *is* the player now), driving the existing, already-built
  `ui/src/lib/engine/{audio-engine.ts,crossfade.ts,mediasession.ts}` (built source-agnostic in
  `offline-local-music-pwa` specifically for this reuse) against `/media/stream/:videoId` instead of
  local `blob:` URLs.
- Wire the music-video path (`VideoSurface`/`NowPlaying`'s video element) against
  `/media/video/:videoId`.
- Wire Listen Together's host-side playback broadcast now that a web client actually has play/
  pause/seek/track-change events to broadcast (`lt-client.ts` already has the WS plumbing; it just
  never had anything to send).
- **Not included**: pitch/tempo (no Web Audio equivalent, dropped entirely — matches the master
  plan and `offline-local-music-pwa`'s own precedent), real OAuth login, mini-player, native
  ambient-video/WebGL compositing, any change to Discord RPC.

## Capabilities

### New Capabilities

- `http-media-streaming`: HTTP surface for resolving a YouTube videoId to a playable stream
  (cipher/PoToken/client-fallback pipeline, Tauri-free) and serving the actual audio/video bytes
  through a throttling-safe range proxy — anonymous-capable, matching search/browse.
- `web-playback`: the browser frontend's playback engine, queue, and transport controls — a
  client-owned reactive store (no server-pushed now-playing/position/queue events for this build)
  driving the existing Web Audio engine against the streaming routes above, including Listen
  Together's host-side broadcast.

### Modified Capabilities

(none — the existing signed-in command surface and the `/ws` event envelope are unchanged; this
change only fills in previously-stubbed playback behavior, it doesn't alter any shipped
requirement.)

## Impact

- New Rust code: `crates/core/src/cipher/` (ported + `jshost.rs` replacing `webview.rs`),
  `crates/core/src/v8root.rs`, `crates/core/src/media.rs` (ported `audioproxy.rs`/`videoproxy.rs`/
  `orchestrator.rs`), new routes in `crates/server`.
- New direct dependency: `deno_core`, pinned to the exact version already resolved transitively via
  `rustypipe-botguard` (`=0.331.0`, resolving to `v8 v130.0.7`) — `cargo tree -i v8` must keep
  showing exactly one `v8` after this change.
- Frontend: `player.svelte.ts` gains real playback-engine responsibility (previously pure
  remote-control, since mpv did the playing on desktop); `api.web.ts`'s transport/queue stubs
  become real; a new client-owned queue store replacing the server-event-driven one `initApp`
  (Tauri) still uses — `initWebApp` (from `signed-in-http-parity`) needs to absorb this.
- `src-tauri` is untouched — same frozen-reference stance as every prior change in this series.
