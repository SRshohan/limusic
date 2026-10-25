## Context

See proposal.md - Why/What Changes for motivation. This section covers only what the approach
below has to work with or around.

- The mockups (`mockups/mobile-redesign/`) are static HTML/CSS/JS with no build step — they prove
  out the visual design and interaction patterns, but every page's "backend" is faked in-page JS
  (e.g. `signin.html`'s `setTimeout`-based fake validation, `downloads.html`'s hard-coded storage
  numbers). None of that logic ports as-is; only the layout, tokens, and interaction patterns do.
- `auth.account.signedIn` (from `getAccount()`) already exists and is read throughout the app
  (`HomeHero.svelte` checks it today for the avatar/greeting). There is no existing route guard
  that redirects an unauthenticated session away from app content — search works anonymously by
  design (`http-search-vertical-slice`), which is exactly the behavior `web-signin-gate` revokes
  for the web build.
- `yt-cache.ts`'s `CacheEntry` has no concept of "don't evict this." `evictIfOverBudget` walks every
  entry oldest-first with no exception.
- The mobile Settings/Profile pages have no existing SvelteKit routes; the closest real equivalents
  are desktop-only (`SettingsDialog.svelte`, `AccountMenu.svelte`).

## Goals / Non-Goals

**Goals:**
- Port the mockups' visual design (tokens, icon, the three new pages' layouts) into real Svelte
  components and routes, reusing existing app infrastructure (API layer, i18n, existing components
  like `ChipRail`) rather than duplicating the mockups' inline CSS/JS.
- Implement the sign-in gate as a real route guard backed by the existing, already-validated
  `signInWithCookie`/`getAccount` — no new validation logic, just a new place that calls it and
  blocks on the result.
- Add persistence (a `pinned` flag) to the existing `yt-cache.ts` store rather than building a
  second, parallel cache for downloads.

**Non-Goals:**
- No change to how a cookie is validated server-side (`state.rs`'s `sign_in` is correct and
  unchanged) — this change only adds a client-side gate that calls the existing path.
- No desktop/Tauri changes: desktop keeps its own sidebar/dropdown account UI and its own sign-in
  model, untouched.
- No video downloads (see proposal.md).
- No change to `offline-local-music-pwa`'s local-file-import feature (`/offline` route,
  `local-files.html` in the mockups) — it already exists and is unaffected by this change.

## Decisions

1. **Gate implementation: a root `+layout.ts`/`+layout.svelte` check, not a per-route guard.**
   Call `getAccount()` once at the root layout; if `!signedIn`, render only the sign-in gate
   component in place of `{@render children()}`, for every route except the gate itself. Rejected
   alternative: a guard in every route's own `+page.ts` — more places to keep in sync, and the
   mockup's own framing ("this gate replaces the whole app shell") maps directly onto one root
   check, not N per-route ones.
2. **`yt-cache.ts` gains `pinned: boolean` on `CacheEntry`, plus `pin(videoId)`/`unpin(videoId)`/
   `listDownloaded()`.** `evictIfOverBudget` skips any entry with `pinned: true` when computing
   what's eligible for eviction. `cacheInBackground` (the existing autoplay-cache path) is
   unchanged — a track becomes a "download" only through an explicit pin, never implicitly through
   normal play.
   **Alternative considered**: a second, separate IndexedDB store just for downloads. Rejected —
   it would duplicate storage for a track that's both recently played and downloaded, and
   `getCached` would need to check two stores on every playback instead of one.
3. **Storage reported on the Downloads page comes from summing `pinned` entries' `bytes`**, not a
   separate running counter — avoids a second number that can drift from the entries actually
   stored.
4. **Settings/Profile/Downloads/sign-in gate become new routes** (`/settings`, `/profile`,
   `/downloads`, and the gate as a component the root layout renders conditionally rather than a
   route, since it must be able to block *any* URL, not just one path) — following the existing
   pattern of `/library`, `/search` as top-level routes.

## Risks / Trade-offs

- [Removing anonymous search/browse for the web build is a real behavior regression for anyone
  currently using it without an account] → Accepted per explicit direction (proposal.md's
  **BREAKING** note) — flagged here so it's a visible, deliberate trade-off in the record, not a
  silent side effect discovered later.
- [A root-layout gate that blocks on `getAccount()` adds a network round trip before any route can
  render, including ones that don't need auth] → Mitigated: `getAccount()` is already called today
  (it's not a new request), this just makes the app wait on its result before rendering instead of
  racing it.
- [Pinned downloads can still fill the 1 GiB cap entirely, leaving no room for the ordinary
  play-cache to do its job] → Out of scope for this change to solve (no admission limit on
  downloads is specified); worth a follow-up if it proves to be a real problem in practice.
- [Porting the mockups' hand-rolled interaction JS (tab switching, theme toggle, chip-rail fade)
  risks drifting from the real app's existing equivalents (`ChipRail.svelte`, the real theme store)
  if done as a literal copy] → Mitigated: tasks.md calls for reusing the existing components/stores
  directly, treating the mockup HTML/JS as a reference for behavior and layout, not as source to
  paste in.

## Migration Plan

Pure frontend/client-storage change (the one `yt-cache.ts` schema addition is a new optional field,
not a breaking IndexedDB version bump — existing entries simply read as `pinned: undefined`/falsy,
treated as "not pinned," no migration step needed). Ship behind no feature flag: the sign-in gate is
the one user-visible behavior change, and it's the explicit point of this proposal, not something to
soften with a flag. Rollback is a normal revert.
