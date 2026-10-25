## Why

The web/PWA build's visual design was never the focus of `mobile-responsive-ui` (that change fixed
layout/overflow bugs in the existing desktop-first look) or `http-search-vertical-slice`/
`offline-local-music-pwa` (those built the underlying server/playback/offline capabilities). A
full set of mobile-first mockups has since been built and iterated on in `mockups/mobile-redesign/`
— a mint-based visual identity, a custom app icon, and several pages the real app doesn't have yet
(a mobile-native Settings page, a Profile/account-switching page, a dedicated cookie-paste sign-in
flow, and a Downloads page for offline YouTube tracks) — and reviewed well enough to move into the
real SvelteKit app.

## What Changes

- Apply the mint design tokens (light mode + a mint-tinted dark mode) from
  `mockups/mobile-redesign/tokens.css` to the real app's `ui/src/routes/layout.css` `@theme`
  tokens, replacing the current color values. Presentation-only — no spec capability, consistent
  with how `mobile-responsive-ui` already separated layout/behavior specs from pure visual choices.
- Replace the app's icon/favicon with the new mark (`mockups/mobile-redesign/assets/icon.svg`).
- **BREAKING**: the web/PWA build requires a valid signed-in session before any route is usable —
  today, search and browsing work anonymously (`http-search-vertical-slice`'s design.md explicitly
  scoped anonymous search as in-bounds). This change removes that anonymous path for the web build;
  a user who hasn't signed in sees only the sign-in gate, nothing else. Desktop/Tauri is unaffected
  (it already requires its own sign-in for anything beyond the unauthenticated bundled-client
  search path, which this change does not touch).
- New mobile Profile page: current account, switch between saved accounts, sign out, and an entry
  point to add another account — reachable from a new avatar button in Home's header. Today this is
  desktop-only, inside `AccountMenu.svelte`'s sidebar dropdown.
- New mobile-native Settings page (grouped list: Account / Appearance / Playback / Integrations /
  About) as the web build's own Settings surface — not a port of the desktop `SettingsDialog.svelte`
  tabbed dialog, which stays desktop-only.
- The existing cookie-paste sign-in (today cramped inside `AccountMenu.svelte`'s dropdown) becomes
  its own full page, reachable from Profile → "Add another account", with room for the real 4-step
  instructions. Steps are also simplified to point at the exact `account_menu` endpoint the backend
  itself validates against, rather than "find any request to music.youtube.com."
- New offline Downloads page and backend support: explicit, user-controlled, persistent offline
  copies of YouTube tracks — distinct from the existing silent, evictable play-cache
  (`ui/src/lib/yt-cache.ts`, LRU, 1 GiB cap). Audio only; video is out of scope (storage cost, niche
  use).
- Library's 7-tab row and the mood/filter chip rows get the shared `ChipRail` scroll+fade component
  `mobile-responsive-ui` already built — no new work, just confirming it's reused rather than
  rebuilt during this migration.
- Out of scope: the desktop/Tauri UI's visual design (unchanged), any change to the innertube/
  streaming pipeline, Listen Together.

## Capabilities

### New Capabilities
- `web-signin-gate`: the web/PWA build requires a valid session before any route renders app
  content; what the gate itself must do (block access, validate the cookie, surface a real failure
  reason, hand off to the app once valid).
- `account-profile`: a mobile-reachable page for viewing the current account, switching between
  saved accounts, signing out, and reaching the add-account flow — behavior currently only
  reachable through the desktop sidebar's dropdown.
- `offline-downloads`: explicit, user-initiated, persistent offline copies of YouTube tracks that
  survive the existing play-cache's eviction, with a UI to see what's downloaded, how much space
  they use, and remove them.

### Modified Capabilities
(none — no existing capability has an archived main spec yet to delta against; see Impact)

## Impact

- Affected UI code: `ui/src/routes/layout.css` (theme tokens), `ui/src/app.html`/manifest icon
  references, `ui/src/lib/components/HomeHero.svelte` (new avatar entry point), new routes/
  components for Profile, Settings (mobile), the sign-in gate, and Downloads, plus whatever
  top-level layout change is needed to actually gate rendering on session state.
- Affected backend: `ui/src/lib/yt-cache.ts` needs a `pinned` flag (and `pin`/`unpin`/`listDownloaded`
  functions) so an explicit download survives the existing LRU eviction; no server-side change
  identified yet (the download itself already goes through the existing `/media/stream/:videoId`
  route).
- No change to `crates/server`/`crates/core` auth validation itself (`state.rs`'s `sign_in` already
  does a real round-trip against YouTube before accepting a cookie — the gate consumes that
  unchanged, it doesn't re-implement it).
- `openspec/specs/` has nothing archived yet for any capability in this project — these are all new
  spec files, not deltas against a main tree that doesn't exist yet.
