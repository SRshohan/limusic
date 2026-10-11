## Context

Limusic's frontend (`ui/`) is a SvelteKit SPA (`adapter-static`, Svelte 5 runes) built for the
Tauri desktop shell: every data/playback action today goes through `@tauri-apps/api` `invoke`
calls into a Rust backend (`src-tauri`) that drives libmpv. That whole stack is Tauri/mpv-specific
and out of scope here — see proposal.md. This change only adds new, mostly self-contained frontend
modules; it does not modify `src-tauri`, `crates/player`, or the existing desktop "Local Music" tab
(`local.rs` / `LocalMusic.svelte`).

This is Phase 1 of a larger plan (`/Users/sohanurrahman/.claude/plans/snug-pondering-kahan.md`) to
turn Limusic into a self-hosted Rust server + PWA, chosen to go first specifically because it needs
none of the later phases' backend work (headless cipher/BotGuard, axum server, frontend transport
rewrite) and is independently valuable on its own. Later phases are out of scope for this change and
will be proposed separately once it is archived; they are mentioned here only so the design choices
below (e.g. a source-agnostic playback engine) are legible as groundwork rather than one-off code.

Implementation work already landed in this session, before this change's artifacts were written,
and is treated as ground truth for the design below rather than re-derived:
- `ui/src/lib/platform.ts`, `ui/src/lib/engine/{crossfade,audio-engine,mediasession}.ts`,
  `ui/src/lib/local/{types,db,import,store.svelte}.ts`
- `ui/vite.config.ts` (`@vite-pwa/sveltekit` wired in), `ui/src/app.d.ts` (PWA virtual-module
  types), `ui/static/icons/pwa-{192,512}.png`
- Dependencies added to `ui/package.json`: `vite-plugin-pwa`, `@vite-pwa/sveltekit`,
  `music-metadata`, `idb`

## Goals / Non-Goals

**Goals:**
- Local-music import, storage, and playback that works with the device's network fully disabled,
  using only browser-native and IndexedDB/OPFS storage — no server, no Tauri.
- A playback engine (`audio-engine.ts`) with no knowledge of *where* its URLs come from, so Phase 4
  (YouTube streaming) reuses it unchanged, pointed at `/media/stream/:id` instead of `blob:` URLs.
- An installable PWA shell that never silently discards playback state on an update.

**Non-Goals:**
- Pitch/tempo adjustment (mpv's rubberband/scaletempo2 filters) — dropped per the overall plan, no
  Web Audio equivalent exists natively.
- A server-side / NAS-hosted local-music mode (reusing `local.rs` over LAN) — listed as a v2
  stretch item in the overall plan, not here.
- Touching the existing Tauri desktop app's own Local Music tab, playback store
  (`player.svelte.ts`), or any `@tauri-apps/api` usage — those are Phase 4 concerns.
- True maskable PWA icons (safe-zone padding) — shipped as `purpose: "any"` only for now; no image
  tooling (ImageMagick/PIL) was available to generate a padded maskable variant.

## Decisions

### Two-deck Web Audio engine, not a single `<audio>` element
**Decision**: `AudioEngine` (`ui/src/lib/engine/audio-engine.ts`) holds two `<audio>` elements, each
through its own `MediaElementAudioSourceNode → GainNode → destination`, mirroring the desktop app's
two-mpv-deck design (`crates/player/src/lib.rs`'s `Decks`). The idle deck is preloaded with the next
track; on end-of-track (or crossfade trigger) playback swaps to it.
**Why**: a single `<audio>` element cannot play two overlapping sources, so crossfading is
impossible with one element. Two decks let the next track start decoding/buffering ahead of time
(gapless) and let both play simultaneously during a fade.
**Alternative considered**: the Web Audio `AudioBufferSourceNode` approach (decode the whole file
into a buffer up front). Rejected: it requires fully fetching and decoding each file before
playback can start, which is wasteful for large local libraries and adds latency `<audio>`'s
streaming playback doesn't have.

### Equal-power crossfade curve via `setValueCurveAtTime`, ported from the Rust fade math
**Decision**: `ui/src/lib/engine/crossfade.ts` ports `fade_due`/`fade_volume` from
`crates/player/src/lib.rs` verbatim for the timing/shape (`cos`/`sin` quarter-turn, capped at half
of either track's duration), but schedules the ramp as one `GainNode.gain.setValueCurveAtTime` call
instead of the Rust side's manual 50 ms-step thread, and drops the `perceptual_to_mpv`/cbrt
conversion since `GainNode.gain` is already linear amplitude (mpv's `volume` property is not — it's
a cube).
**Why**: reusing proven timing/shape math avoids re-deriving fade behavior, while
`setValueCurveAtTime` is the idiomatic, jank-free way to schedule a continuous ramp in Web Audio
(no JS timer tick driving it).

### Local storage: File System Access handles where available, OPFS copy as fallback
**Decision**: `ui/src/lib/local/import.ts` prefers `showDirectoryPicker`/`FileSystemFileHandle`,
persisting the handle itself in IndexedDB (`ui/src/lib/local/db.ts`) so a rescan only needs a
permission re-grant, not a re-pick, and so file bytes are never duplicated into browser storage.
The plain-`<input>` fallback (Firefox/Safari, or a one-off file selection) has no durable handle, so
those files are copied into OPFS (`navigator.storage.getDirectory()`) once, at import time, so the
library survives a reload.
**Why**: a multi-GB local library duplicated into IndexedDB/OPFS would be wasteful and slow to
import; a live handle reads the original file each time. OPFS is only a fallback for the browsers/
import paths that have no better option.
**Alternative considered**: always copy into OPFS. Rejected as the default — needlessly doubles
disk usage and import time on every browser that supports the better API.

### IndexedDB schema: four stores, no server-shaped `AppState`
**Decision**: `sources`, `tracks` (indexed by `sourceId`), `playlists`, `playlistTracks` (indexed by
`playlistId` and `trackId`) — directly mirroring the desktop app's `local_tracks`/`local_playlists`/
`local_playlist_tracks` SQLite tables conceptually, but as the only copy of the data (no
server-side mirror).
**Why**: keeps the mental model aligned with the existing Rust schema (easing a future "also
support a server-hosted shared library" mode) without actually depending on it.

### Media Session API instead of any native integration
**Decision**: `ui/src/lib/engine/mediasession.ts` wires `navigator.mediaSession` actions/metadata/
position-state directly to `AudioEngine`, with no native module involved.
**Why**: this is a strict scope reduction from `souvlaki`/`media.rs` (OS media-key daemons, MPRIS,
SMTC) — a real browser forwards hardware/OS media-key presses to the page automatically once
`MediaSession` is set, so there is nothing else to build.

### PWA plugin: `@vite-pwa/sveltekit` (Workbox `generateSW`), manual update prompt
**Decision**: `ui/vite.config.ts` configures `SvelteKitPWA` with `injectRegister: false` and a
`globPatterns`-based app-shell precache; `navigateFallbackDenylist` and a scoped `runtimeCaching`
rule keep any future `/media/`-prefixed or `/ws` route out of the cache entirely. Registration and
the update-available prompt are done manually via the `virtual:pwa-register/svelte` composable
(`useRegisterSW`), gated to run only outside the Tauri webview (`platform.ts`'s `isTauri`).
**Why**: `generateSW` (vs. `injectManifest`) needs no hand-written service worker for a shell with
no custom runtime logic beyond what Workbox's recipes already cover. Manual registration (vs. the
plugin's own auto-injected script) is required to (a) gate it off inside Tauri, where a service
worker is unwanted and untested against the desktop webview, and (b) show an update prompt instead
of silently reloading mid-playback.

## Risks / Trade-offs

- **[Risk]** File System Access API is Chromium-only; Firefox/Safari users get the `<input>`
  fallback, which cannot "watch" a folder for changes and needs explicit re-import for new files.
  → **Mitigation**: `rescan` is still available for the folder-handle path on Chromium, and the
  spec explicitly scopes the fallback's guarantee to "selected files remain playable," not
  "the folder stays live."
- **[Risk]** A permission grant on a `FileSystemFileHandle`/`FileSystemDirectoryHandle` can be
  revoked by the browser between sessions, surfacing as a playback failure if not handled.
  → **Mitigation**: `resolveTrackFile`/`rescanSource` check `queryPermission` and prompt via
  `requestPermission` before failing; the spec has an explicit scenario for this.
- **[Risk]** Service worker caching, done wrong, could serve a stale app shell indefinitely or
  (worse) intercept a future streamed-media request and serve garbage.
  → **Mitigation**: `registerType: 'prompt'` (not silent auto-update) plus an explicit
  `navigateFallbackDenylist`/`runtimeCaching` scope that only ever touches the shell and `/api/*`
  reads — enforced by a spec requirement, not just vite.config.ts comments.
- **[Risk]** Large embedded cover art (some files carry multi-MB images) could bloat IndexedDB
  across a big library. → **Mitigation**: `import.ts` already skips storing an embedded cover over
  ~400KB rather than capping library size; tracked as a note in the code, not a hard spec
  requirement since it's a storage-efficiency concern, not observable behavior.

## Migration Plan

Purely additive — no existing route, store, or Tauri code path is modified or removed. Shipping
this change does not change the desktop app's behavior at all (new modules are inert until the new
route/nav entry is reached, and that entry only renders meaningfully outside Tauri). No rollback
concerns beyond reverting the new files.
