## Why

Limusic is a Tauri desktop app today; the goal is to turn it into installable, free music
software reachable from a plain browser. The user also wants music that plays with **no internet
connection at all** — not just no YouTube, but no network reachability of any kind, including no
LAN to a self-hosted server. That rules out routing local files through any backend as the only
option, so this first slice delivers a fully client-side, installable PWA for the user's own audio
files: pick a folder or files once, then play them — gapless, crossfaded, with lock-screen/media-key
controls — in airplane mode, forever. It ships before any of the higher-risk YouTube-streaming
backend work (cipher/BotGuard headless hosting, axum server, etc. — see the follow-on changes this
one unblocks) because it needs none of that, is independently valuable, and proves out the browser
playback engine (`audio-engine.ts`/`crossfade.ts`/`mediasession.ts`) that the YouTube-streaming path
reuses unchanged later.

## What Changes

- Add a client-side local-music library: pick a folder (File System Access API, persistent
  handle) or pick files (`<input type=file>` fallback, copied into OPFS so the import survives a
  reload); extract ID3/tag metadata in-browser; index tracks, sources, and playlists in IndexedDB.
  Nothing in this path ever calls a server — that is what makes it work with zero network.
- Add a browser playback engine: two `<audio>` elements through a Web Audio graph (`GainNode` per
  deck) reproducing the desktop app's gapless/crossfade behavior, with the equal-power fade curve
  ported from `crates/player/src/lib.rs`. Wired to the Media Session API for OS/lock-screen
  transport controls (play/pause/seek/next/prev), replacing what `souvlaki`/`media.rs` did natively.
- Add a PWA shell: Web App Manifest, service worker (Workbox via `@vite-pwa/sveltekit`,
  `generateSW` strategy), installable icons, and an update-available prompt — precaching the app
  shell only, explicitly never caching any future streamed-media route.
- Add a new UI surface (route + nav entry) for browsing/playing the local library, built on the
  existing component patterns (`TrackRow`, `MediaCard`, etc.) but driven by the new client-side
  store instead of Tauri `invoke`/mpv.
- The existing Tauri desktop build and its own "Local Music" tab (`local.rs`, `LocalMusic.svelte`)
  are untouched — this change is additive, gated so the new PWA-only surface only engages outside
  the Tauri webview (`isTauri` check).

## Capabilities

### New Capabilities
- `offline-local-music`: client-side import, storage, and gapless/crossfade playback of the
  user's own audio files, requiring zero network/server reachability once installed.
- `pwa-installability`: the app is installable from a browser and precaches its own shell via a
  service worker, with a user-facing update-available prompt.

### Modified Capabilities
_None — no existing spec-level behavior changes. The Tauri app's current "Local Music" tab and its
Rust-side `local.rs` are unmodified by this change._

## Impact

- New frontend-only modules under `ui/src/lib/engine/`, `ui/src/lib/local/`, plus a new route
  under `ui/src/routes/`. No Rust changes, no server (none exists yet — later changes add it).
- New dependencies in `ui/package.json`: `vite-plugin-pwa`, `@vite-pwa/sveltekit`,
  `music-metadata`, `idb`.
- `ui/vite.config.ts` and `ui/src/app.d.ts` gain PWA-plugin wiring; `ui/static/icons/` gains
  generated PWA icon assets.
- `ui/src/lib/components/Sidebar.svelte` gains one nav entry, gated to non-Tauri.
- This is the first of several planned changes converting Limusic into a self-hosted-server + PWA
  (see `/Users/sohanurrahman/.claude/plans/snug-pondering-kahan.md` for the full multi-phase plan);
  later changes (HTTP core, headless cipher/streaming, frontend transport rewrite, PWA polish) are
  out of scope here and will be proposed separately once this one is archived.
