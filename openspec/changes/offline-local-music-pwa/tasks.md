## 1. Foundations (already done this session)

- [x] 1.1 Platform detection (`isTauri`/`supportsFsAccess`/`supportsOpfs`) — `ui/src/lib/platform.ts`
- [x] 1.2 Crossfade timing/curve math ported from `crates/player/src/lib.rs` — `ui/src/lib/engine/crossfade.ts`
- [x] 1.3 Two-deck Web Audio gapless/crossfade playback engine — `ui/src/lib/engine/audio-engine.ts`
- [x] 1.4 Media Session (lock-screen/OS transport) wiring — `ui/src/lib/engine/mediasession.ts`
- [x] 1.5 Local-library types + IndexedDB schema — `ui/src/lib/local/types.ts`, `ui/src/lib/local/db.ts`
- [x] 1.6 Import/rescan/resolve: File System Access API + OPFS fallback — `ui/src/lib/local/import.ts`
- [x] 1.7 Reactive store tying library + queue + engine together — `ui/src/lib/local/store.svelte.ts`
- [x] 1.8 PWA plugin wired into `ui/vite.config.ts` (`@vite-pwa/sveltekit`, manifest, Workbox scoping, `injectRegister: false`)
- [x] 1.9 PWA virtual-module type references added — `ui/src/app.d.ts`
- [x] 1.10 PWA icons generated from `src-tauri/icons/icon.png` — `ui/static/icons/pwa-192.png`, `pwa-512.png`
- [x] 1.11 Dependencies installed — `vite-plugin-pwa`, `@vite-pwa/sveltekit`, `music-metadata`, `idb` (in `ui/package.json`)

## 2. Local-music UI

These are independent of each other except where noted — pick any.

- [x] 2.1 Import UI: "Add folder" / "Add files" controls calling `addFolder()`/`addFiles()` from
      `local/store.svelte.ts`, plus a list of imported sources with rescan/remove buttons. Verify:
      picking a folder populates the source list and its tracks appear in `local.tracks`. —
      `ui/src/lib/components/offline/OfflineImport.svelte`
- [x] 2.2 Library browsing views (songs/albums/artists tabs), following the existing
      `LocalMusic.svelte` pattern (`TrackFilter`, tabs) but reading from `local.tracks` instead of
      `api`/Tauri `invoke`, with lightweight self-contained row/card markup rather than reusing
      `MediaCard`/`TrackRow` directly (those are wired to YouTube-specific navigation/menus via
      `$lib/browse`, which assumes a Tauri backend — reusing them as-is would have pulled in
      broken navigation). Grouped client-side by `album`/`artist`. Verify: a library of a few dozen
      imported tracks renders correctly grouped, text filter narrows all three tabs. —
      `ui/src/lib/components/offline/OfflineLibrary.svelte`
- [x] 2.3 Playback transport UI: play/pause, seek bar, next/prev, volume, and a crossfade-seconds
      setting, bound to `playback` state and actions from `local/store.svelte.ts`. Verify:
      scrubbing the seek bar seeks the active track; toggling crossfade from 0 to >0 changes
      observed behavior at a track boundary. — `ui/src/lib/components/offline/OfflineTransport.svelte`
- [x] 2.4 New route `ui/src/routes/offline/+page.svelte` assembling 2.1–2.3 into one page. Verify:
      `pnpm build && pnpm preview`, loading `/offline` in a plain browser (confirmed via headless
      Chrome `--dump-dom`: renders "Offline music", import controls, empty-library state — no crash).
- [x] 2.5 Entry point to `/offline` for the browser case. **Changed from the originally-planned
      Sidebar nav item** — see the added-scope note below; implemented instead as a redirect from
      `/` when `!isTauri` — `ui/src/routes/+page.ts`. Verify: headless-Chrome `--dump-dom` of `/`
      shows the Offline music page content.
- [x] 2.6 Local playlists UI: create/rename/delete a playlist, add/remove a track, backed by the
      existing `playlists`/`playlistTracks` stores in `local/db.ts` (store gained
      `createPlaylist`/`renamePlaylist`/`deletePlaylistById`/`getPlaylistTrackIds`/
      `addTrackToPlaylist`/`removeTrackFromPlaylist` wrappers). —
      `ui/src/lib/components/offline/OfflinePlaylists.svelte`

### Added scope found while implementing (not in the original task list)

Discovered the root layout (`ui/src/routes/+layout.svelte`) unconditionally calls Tauri-only APIs
that **throw synchronously** with no Tauri runtime present — `getCurrentWindow()` (used directly,
and inside `Titlebar.svelte`/`ResizeBorders.svelte`, both rendered unconditionally around every
route). This blocked the entire app, including `/offline`, from loading in a plain browser at all,
not just a missing nav item. Surfaced to the user and resolved per their chosen option ("minimal
shell outside Tauri"):
- `+layout.svelte`: `isMini` computation and `loadAppIcon()`/`initWin()`/`checkForUpdatesQuiet()`/
  `initZoom()` calls are now gated behind the existing `isTauri` helper (zero behavior change for
  the Tauri build, since `isTauri` is `true` there).
- Root template gained a third branch (mini / **plain-browser minimal shell** / full Tauri chrome):
  outside Tauri, `Titlebar`/`ResizeBorders`/`Sidebar`/`PlayerBar` are not rendered at all — just the
  page content plus `PwaUpdatePrompt` — since those components are either Tauri-crash-prone or
  wired to a backend that doesn't exist yet for this surface.
- `ui/vite.config.ts`: added the missing `injectRegister: false` (design.md already described this,
  but it had been left out of the actual config) so the plugin doesn't auto-inject its own register
  script alongside our manual, `isTauri`-gated one.

## 3. PWA update UX

- [x] 3.1 Update-available prompt component using `virtual:pwa-register/svelte`'s `useRegisterSW`
      (`needRefresh`/`updateServiceWorker`), gated to `!isTauri`, mounted from the root layout. —
      `ui/src/lib/components/PwaUpdatePrompt.svelte`. Required an additional dependency not listed
      in the original plan: `workbox-window` (peer dependency the composable imports at runtime;
      the build fails without it).
- [ ] 3.2 Confirm the manifest link and service worker registration are skipped entirely when
      `isTauri` is true. Verify: run the existing Tauri dev build (`cargo tauri dev`) and confirm
      no service worker registers and no install prompt appears inside the desktop webview. **Not
      run this session** — needs an actual Tauri/Rust dev build, which wasn't exercised here.

## 4. Verification

- [x] 4.1 `pnpm check` (svelte-check + tsc) passes with no new errors. Required one fix beyond the
      original plan: TypeScript's bundled DOM lib declares `FileSystemFileHandle`/
      `FileSystemDirectoryHandle` but not their `queryPermission`/`requestPermission` methods
      (a Chromium-only incubating extension); `@types/wicg-file-system-access` was already listed
      as a dependency to add, but needed an explicit `/// <reference types="wicg-file-system-access" />`
      in `import.ts` — automatic `@types` discovery alone wasn't picking it up.
- [x] 4.2 `pnpm build` succeeds; `ui/build` contains `sw.js`, `workbox-*.js`, and
      `manifest.webmanifest`, and `index.html` now links the manifest (added directly to
      `ui/src/app.html` — the plugin's automatic `<link rel="manifest">` head injection doesn't
      reach this project's `ssr=false`/adapter-static fallback shell, confirmed empty before the fix).
      Runtime-smoke-tested with `pnpm preview` + headless Chrome: `/`, `/offline`,
      `/manifest.webmanifest`, `/sw.js` all 200; `/` redirects to and renders the Offline music page.
- [ ] 4.3 Manual: import a folder of a few tracks, play the queue, confirm no audible gap between
      tracks with crossfade set to 0. **Needs a real browser + real audio files — not done this
      session.**
- [ ] 4.4 Manual: set crossfade to a few seconds, confirm an audible overlapping fade (not a cut)
      at a track boundary, and that it never audibly runs past half of either track's length.
- [ ] 4.5 Manual: with a track playing and the browser tab backgrounded, confirm lock-screen/OS
      media controls (play/pause/next/prev) work and show correct title/artist/art.
- [ ] 4.6 Manual: install the PWA, enable airplane mode, fully close and reopen it, confirm the
      shell loads and a previously imported track plays with no network-related error.
- [ ] 4.7 Manual: ship a second build behind the same URL, confirm the update prompt appears
      without interrupting an in-progress track, and the new version activates only after the user
      acts on the prompt.
