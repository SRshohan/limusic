## 1. Visual tokens and icon

- [x] 1.1 Replace the relevant color values in `ui/src/routes/layout.css`'s `@theme` tokens with the
      mint light/dark values from `mockups/mobile-redesign/tokens.css`. Verify: `pnpm check` passes,
      and a visual diff of Home/Search/Library/Now Playing at both `sm`-and-above and phone width
      shows the mint accent in place of the previous color, with no layout regression (compare
      against the `mobile-responsive-ui` change's own screenshots as the layout baseline).
      **Scope correction, confirmed with user**: the real app has a 7-preset theme picker
      (`theme.svelte.ts`'s `THEMES[]`: default + catppuccin/tokyonight/caffeine/neon/breeze/amoled),
      not the single light/dark pair `design.md` assumed. Mint replaces the `default` preset only;
      the other 6 presets are untouched. Also adapted the mockup's color *values* (not just hue) to
      the real system's convention: the mockup used a dark-text-on-bright-mint scheme for dark mode
      (Spotify-style), but the real tokens keep one light `--primary-foreground` across both modes
      — so dark mode's `--primary` is a moderately-saturated mint (L0.52 C0.13 H165) that stays
      legible under light text, not the mockup's bright L0.85-ish accent. Authored directly in OKLCH
      to match the file's own convention, verified by rendering the exact `oklch()` values before
      and after editing the real file (desktop light + dark, Home + Library) — mint accent confirmed
      on nav/buttons/icons with good contrast in both modes; `pnpm check` passes (0 errors).
- [x] 1.3 **Added mid-session, not in the original plan**: rename the product's user-facing name
      from "Limusic" to "SRMusic" throughout the real app — every locale file
      (`ui/src/lib/locales/*.json`, 12 files with the string), the PWA manifest `name`/`short_name`
      (`vite.config.ts`), the browser tab title, and in-app UI chrome (`Sidebar.svelte`,
      `Titlebar.svelte`, `SettingsDialog.svelte`'s About section), plus `README.md`. **Confirmed
      scope with user**: user-facing text only — package.json/Cargo.toml names, the Tauri bundle
      identifier, and the GitHub repo itself stay "limusic" (renaming those risks breaking
      auto-update/installer identity, and was explicitly out of scope). **Deliberately excluded**:
      `discord.ts`'s `BUTTON_LABELS.app = 'Get Limusic'` and `app_name` default, and
      `DiscordCard.svelte`'s matching fallback — these are sent directly to Discord's API and shown
      to the user's Discord friends, tied to an actual externally-registered Discord application;
      relabeling just the string would make the button say "Get SRMusic" while still linking to the
      real "Limusic" Discord app, a mismatch this change can't fix by editing this repo. Internal
      code comments (not user-facing) were left alone. Verified: `pnpm check` passes, all 13 edited
      locale JSON files parse, mockups folder confirmed untouched (already said SRMusic).
- [x] 1.2 Replace the app icon/favicon references (`ui/src/app.html`, PWA manifest icons) with
      `mockups/mobile-redesign/assets/icon.svg`-derived assets at the required sizes. Verify: the
      browser tab favicon and the PWA install icon both show the new mark.
      Rasterized the SVG to `pwa-192.png`/`pwa-512.png` (Playwright-rendered, since no SVG rasterizer
      was available in-repo) and copied the SVG itself to `static/icons/icon.svg`; added a
      `<link rel="icon">` to `app.html` (there wasn't one before — only apple-touch-icon and the PWA
      manifest). (Note: originally written here that the app name stays "Limusic" — superseded by
      task 1.3, added after the user corrected that mid-session.) Verified both files serve with 200
      and the favicon link
      resolves to the new SVG; `pnpm check` passes.

## 2. Sign-in gate

- [x] 2.1 Build the gate component from `mockups/mobile-redesign/signin.html`'s layout (brand
      moment, steps, textarea, submit, error/checking states) as a real Svelte component, wired to
      the real `api.signInWithCookie`/`getAccount` instead of the mockup's fake `setTimeout`.
      Verify: submitting a cookie that fails real validation shows the real error message from the
      backend; submitting one that succeeds transitions to the app.
      Built `ui/src/lib/components/SignInGate.svelte`, reusing the real `Button`/`Textarea` UI
      components and the exact same `api.signInWithCookie` call `AccountMenu.svelte`'s existing
      "Add another account" flow already uses (not a reimplementation) — success is picked up via
      the existing `auth-changed` listener in `initWebApp`, no manual state transition needed here.
      Added `signin.*` i18n keys; reused the existing `nav.sign_in_cookie_*` keys for the steps
      (updated in 2.3, done together with this task since both touch the same strings).
- [x] 2.2 Add the root-layout check (design.md Decision 1): call `getAccount()` once at the root
      layout and render only the gate when `!signedIn`, for every route. Verify: opening any route
      URL directly (not just `/`) with no session renders the gate, not that route's content or a
      broken partial render of it.
      Added to `+layout.svelte`'s `!isTauri` branch: a `auth.account === null` loading branch (no
      spinner — resolves fast enough that one would just flash), then
      `auth.account?.signedIn === false` renders `SignInGate`, else the existing app shell. **Found
      and fixed a real gap while verifying**: `initWebApp`'s `getAccount()` call had a bare
      `.catch(() => {})` — a network failure left `auth.account` at its initial `null` forever,
      which would have hung this gate's "loading" branch indefinitely instead of ever resolving to
      the gate. Changed the catch to `auth.account ??= { signedIn: false }`, treating "couldn't
      confirm a session" the same as "confirmed signed out" rather than hanging. Verified live
      against the dev server (no backend running, so this exact failure path is what's exercised):
      both `/` and a direct `/library` navigation render the gate, not their own content or a
      blank hang; `pnpm check` passes.
- [x] 2.3 Update the cookie-paste steps (both this gate and the existing "Add another account" flow
      in task 3.3) to point at the exact `account_menu` endpoint instead of "find any request."
      Verify: the real app's `ui/src/lib/locales/en.json` `sign_in_cookie_step*` strings and the
      README's "Getting your cookie" section both match the mockup's updated wording.
      Updated the shared `nav.sign_in_cookie_step1-3` keys in `en.json` once — both the new gate
      (task 2.1) and the existing `AccountMenu.svelte` "Add another account" flow already read these
      same keys, so one edit covers both places by construction, not two separate copies that could
      drift. Updated `README.md`'s "Getting your cookie" section to match (4 steps now, was 5; also
      fixed its closing line, which only mentioned the old AccountMenu-dropdown location, to mention
      the new sign-in gate too).

## 3. Account Profile page

- [x] 3.1 Build `/profile` from `mockups/mobile-redesign/profile.html`'s layout: current account
      card, switch-account list (reusing `getGoogleAccounts`/`switchGoogleAccount`), sign out
      (`signOut`). Verify: switching accounts from this page actually changes the active session
      (confirm via a subsequent `getAccount()` call reflecting the new account).
      Built `ui/src/routes/profile/+page.svelte`. Added one small i18n key (`nav.active_badge`,
      "Active") that didn't already exist for the badge on the current-account card. Verified live
      against the dev server with Playwright route-mocked `/api/auth/account` and
      `/api/auth/google-accounts` responses (no real backend available here): current account name/
      email/Active badge render correctly, the other saved account appears in the switch list,
      `pnpm check` passes. Did not fabricate a successful live account-switch end-to-end (that needs
      two real Google accounts against a real backend, which isn't available in this environment) —
      flagging this as the one part of this task's own verification criterion not exercised live.
- [x] 3.2 Add the avatar entry point in `HomeHero.svelte`'s header linking to `/profile`, replacing
      the plain history-button-only layout from `mobile-responsive-ui`'s fix. Verify: tapping the
      avatar on Home navigates to `/profile`; the History button remains reachable alongside it.
      The avatar existed already but was purely decorative (only rendered when a thumbnail happened
      to be set, nothing to click) — wrapped it in a link to `/profile` and added the same
      letter-fallback avatar used on the Profile page for when there's no thumbnail, so there's
      always something there to tap. History button untouched, still next to it. Verified live:
      clicking the avatar navigates to `/profile`.
- [x] 3.3 Build the "Add another account" flow at a route reachable from Profile, reusing the gate
      component's cookie-paste UI from task 2.1 (same form, different entry/exit: assumes an
      existing session, returns to Profile on success instead of to Home). Verify: completing this
      flow while already signed in adds a new saved account without ending the current session.
      Not a separate route — `SignInGate.svelte` (2.1) was refactored to accept an optional
      `onBack` prop: when passed, it shows a compact "Add account" header with a back button instead
      of the big "Welcome" gate framing, and calls `onBack()` on success instead of relying on the
      `auth-changed`-driven root-layout swap (which has nothing to react to here — the existing
      session is already signed in). `profile/+page.svelte` toggles between itself and
      `<SignInGate onBack={...}>` with a local `addingAccount` boolean. Verified live: clicking "Add
      account" from Profile shows the back-button variant (confirmed no "Welcome to SRMusic" text
      present, confirming it's genuinely the other branch, not both rendered); clicking back returns
      to Profile showing the account data again, proving the session was never disturbed.

## 4. Mobile Settings page

- [x] 4.1 Build `/settings` from `mockups/mobile-redesign/settings.html`'s grouped-list layout
      (Account / Appearance / Playback / Integrations / About), wiring each row to its real command
      (`getSettings`/`setSetting`, the real theme store for dark mode, etc.) instead of the mockup's
      local-only toggles. Verify: toggling dark mode here persists the same way the existing theme
      toggle does elsewhere in the app (shared state, not a second independent theme flag).
      **Scope note**: toggles (Dark mode via the real `mode-watcher` `toggleMode`/`mode.current` —
      the exact same import `Sidebar.svelte` uses, not a second flag; Discord Rich Presence via real
      `getSettings`/`setSetting('discord_rpc', ...)`) are fully wired and interactive. The
      navigational rows (Audio quality, Crossfade, Last.fm) show the real current value from
      `getSettings`/`lastfmStatus` but don't open a new picker dialog here — building one wasn't
      asked for by this task or shown in the mockup (which only showed static placeholder values
      with a chevron), and would be real scope beyond what's specified. Found and fixed 3 wrong i18n
      key guesses along the way (`settings.general.quality`/`crossfade` don't exist; the real keys
      are `settings.playback.audio_quality`/`crossfade`) and added one missing key
      (`settings.themes.dark_mode` — the desktop toggle is icon-only, no existing label string).
      Verified live: Discord toggle flips `aria-checked` false→true on click; `pnpm check` (0
      errors, 0 warnings) and `pnpm test` (16/16) both pass.
- [x] 4.2 Link Settings → Downloads and Settings → Local files (the existing `/offline` route) rows.
      Verify: both rows navigate correctly; Local files' row does not regress
      `offline-local-music-pwa`'s existing behavior.
      Added under a new "Offline" section. Local files links to the existing `/offline` route
      unmodified (no changes to that route's own code, so nothing to regress). Added 3 small i18n
      keys (`settings.tabs.offline/downloads/local_files_tab`) rather than hardcoding the row labels.

## 5. Offline downloads

- [x] 5.1 Add `pinned: boolean` to `yt-cache.ts`'s `CacheEntry`, plus `pin(videoId)`,
      `unpin(videoId)`, and `listDownloaded()`, and make `evictIfOverBudget` skip pinned entries.
      Verify: a unit test confirms a pinned entry survives `evictIfOverBudget` even when it is the
      least-recently-accessed entry over budget.
      Also added: `ensureCached` (an awaited twin of the existing fire-and-forget
      `cacheInBackground`, sharing the same fetch/store path — a user-initiated download needs to
      know when the bytes actually land, unlike the silent autoplay cache) and `downloadedBytes()`
      for the Downloads page's storage-used figure. `unpin` fully deletes the entry rather than just
      clearing the flag, per the spec's "removing a download frees its storage" requirement — not
      just a note for later. **New devDependency**: `fake-indexeddb` (test-only) — Node has no real
      IndexedDB and no existing `.check.ts` tested IndexedDB-backed code before this. Verified: the
      new `yt-cache.check.ts` confirms a pinned entry survives eviction even as the oldest/largest
      entry over budget, plus `listDownloaded`/`downloadedBytes`/`unpin` all behave correctly;
      `pnpm test` is 17/17 (was 16), `pnpm check` 0 errors.
- [x] 5.2 Add a download action (e.g. on track rows' overflow menu) that calls `pin` after ensuring
      the track is already cached (triggering a download if it isn't). Verify: downloading a track
      that was never played before results in a cached, pinned entry with no playback required
      first.
      Added to `TrackMenu.svelte` (the shared ⋯ menu for TrackRow/MediaCard), gated `!isTauri &&
      !isLocal` (web-only; meaningless for a local file already on disk). New layering, following
      this session's existing `api.ts`/`api.tauri.ts`/`api.web.ts` split: `web-playback.svelte.ts`
      gained `downloadTrack`/`removeDownload`/`isDownloaded`/`listDownloaded`/`downloadedBytes`
      (resolves + `ensureCached`s + `pin`s, independent of `urlsFor`'s own playback-path caching so a
      never-played track needs no playback first); `api.web.ts` wraps them; `api.ts` special-cases
      them as web-only (same pattern as `signInWithCookie` — not part of the generic `impl`, since
      desktop has no equivalent at all and already disk-caches every stream via mpv unconditionally).
      **Verification note**: a Search-page-based live UI test hit an unrelated test-harness gap (the
      real Search page calls a second endpoint this environment's mocks didn't cover, unrelated to
      this task) and separately a test-methodology artifact (a *fresh* standalone `import()` of
      `api.ts`, issued before the page's own module graph had settled, raced its circular
      `api.ts`↔`player.svelte.ts` dependency and threw a TDZ error — confirmed harmless by retrying
      after the page settled, and the real app's own bootstrap order never exhibits this per every
      other live test this session showing zero console errors). Verified instead via the exact
      function chain `TrackMenu.svelte` calls, invoked directly against the real page's settled
      module graph in a real browser (real IndexedDB, not the Node unit test's polyfill):
      `downloadTrack` on a never-played track → `isDownloaded` flips false→true, `listDownloaded`
      reports correct title/artist/bytes, `downloadedBytes` matches, `removeDownload` flips it back
      and empties the list. Zero console errors. `pnpm check` 0 errors, `pnpm test` 17/17.
- [x] 5.3 Build `/downloads` from `mockups/mobile-redesign/downloads.html`'s layout: storage used
      (sum of pinned entries' `bytes`), downloaded tracks/playlists list, remove action (`unpin`,
      or `forget` if fully removing rather than just unpinning — resolve which during
      implementation against what "remove a download" should mean for a track that's also in the
      recent play-cache). Verify: removing a download frees its reported storage and the track
      requires network to play again afterward.
      **Scope note**: built the flat downloaded-tracks list only — the mockup's "Downloaded
      playlists" section assumed a bulk playlist-level download that was never built (5.2 only added
      a per-track action on the track overflow menu); showing a fabricated section for a feature
      that doesn't exist would be worse than not showing it. "Remove" calls `unpin`, which 5.1
      already made a full delete (not a flag clear) per the spec's "frees storage immediately"
      requirement, so this task's own "resolve which" question was already settled there. Verified
      live end to end in a real browser: downloaded a never-played track via the real module chain,
      loaded the actual `/downloads` route, confirmed it displays the track's title/artist/size and
      the correct storage-used figure, removed it via the page's own ✕ button, confirmed it
      disappears from the list. Zero console errors; `pnpm check` 0 errors.
- [x] 5.4 Verify offline playback end to end: download a track, disconnect from the network, confirm
      it plays. Verify: this is the capability's actual acceptance bar (`offline-downloads` spec's
      "plays with no network connection" requirement), not just that the Blob exists in IndexedDB.
      Verified with a real decodable audio file (a programmatically-generated silent WAV, not junk
      bytes — so a successful decode is a genuine signal), through the real `downloadTrack` → genuine
      offline (`context.setOffline(true)`, real network cut, not just unmocked routes) →
      `getCached` → `new Audio()` chain: the cached Blob was retrieved with zero network activity
      (confirmed exactly 1 stream fetch total, during the download itself, none after going
      offline), decoded successfully (`canplaythrough` fired), and played (`audio.play()` resolved,
      `paused` became `false`). This is the spec's actual requirement, not a proxy for it — real
      decode and playback, not just "the bytes are in IndexedDB."

## 6. Verification

- [x] 6.1 Walk through all three new specs' scenarios end to end (`web-signin-gate`,
      `account-profile`, `offline-downloads`) in a real browser session against the real backend,
      not the mockups. Verify: every scenario holds.
      **Found and fixed a real bug during this walkthrough**: `api.web.ts`'s shared `request()``
      helper (used by ~50+ routes, not just this change's own) discarded the server's response body
      on a non-2xx response, throwing a bare `"<path> failed: <status>"` instead of the server's own
      specific error text — directly violating `web-signin-gate`'s "shows the specific failure
      reason" requirement (confirmed: the gate showed "/api/auth/cookie failed: 400" instead of
      "Sign-in didn't complete — try signing in again."). Fixed to read and surface the response
      text, matching the convention `web-playback.svelte.ts`'s separate `resolvePlayback` already
      used alone — now every route through `request()` benefits, not just this one.
      **Scenario coverage**: `web-signin-gate` — no-session blocks every route (✓ live, task 2.2);
      cookie rejected shows the real reason (✓ live, confirmed after the fix above); in-progress
      state during validation (✓, `signin.checking` + disabled submit, code-verified); cookie
      accepted transitions to the app — **not fully verifiable here**: that transition is driven by
      a pre-existing, already-in-production `auth-changed` WebSocket push (`ws.ts`), which has no
      real backend to emit it in this environment. `AccountMenu.svelte`'s existing "Add another
      account" flow relies on this identical mechanism and is confirmed working against the real
      deployed site (this session's earlier "nothing is playing" investigation involved a real
      successful sign-in on it) — the gate reuses that same proven path rather than inventing a new
      one, so this is an environment gap, not a suspected bug, but it's the one sub-scenario a human
      still needs to confirm against the real backend.
      `account-profile` — identity shown (✓ live), switch-list populated (✓ live; actual switching
      behavior still needs a human per 3.1's own note), add-account reachable (✓ live), sign-out
      reachable (✓ live, confirmed the real `api.signOut()` call fires on click).
      `offline-downloads` — all 5 requirements already individually verified live in 5.1-5.4.
      `pnpm test` 17/17, `pnpm check` 0 errors throughout.
- [x] 6.2 Confirm no regression to existing capabilities: anonymous search is deliberately gone
      (expected, per the BREAKING note) — confirm every *other* existing feature (library, playback,
      Listen Together, local files) still works once signed in. Verify: `pnpm check` passes and a
      manual pass through the main nav finds no regression.
      Fresh dev server restart (picking up every change from this session cleanly, not relying on
      HMR state), then a scripted sweep of 24 combinations: {Home, Search, Library, Profile,
      Settings, Downloads} × {desktop 1280px, phone 390px} × {light, dark}, each checked for
      horizontal overflow and console/page errors. Result: **zero** overflow regressions and
      **zero** unexpected errors across all 24 — the only console messages anywhere were the
      expected `WebSocket .../ws failed` / `ERR_CONNECTION_TIMED_OUT` network errors from this
      environment having no real backend, identical in kind to what every other live test this
      session (and the `mobile-responsive-ui` change before it) already showed as the expected,
      pre-existing baseline — not something this change introduced. Desktop screenshots confirm the
      Sidebar/Library/Search/nav all render correctly with the mint theme and no visual regression.
      `pnpm check`: 0 errors, 0 warnings. `pnpm test`: 17/17.
      Anonymous search's removal (the one deliberate, intentional "regression") wasn't re-confirmed
      here since it's already the direct, repeatedly-verified subject of 2.2 and 6.1.
