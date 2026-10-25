## 1. Visual tokens and icon

- [ ] 1.1 Replace the relevant color values in `ui/src/routes/layout.css`'s `@theme` tokens with the
      mint light/dark values from `mockups/mobile-redesign/tokens.css`. Verify: `pnpm check` passes,
      and a visual diff of Home/Search/Library/Now Playing at both `sm`-and-above and phone width
      shows the mint accent in place of the previous color, with no layout regression (compare
      against the `mobile-responsive-ui` change's own screenshots as the layout baseline).
- [ ] 1.2 Replace the app icon/favicon references (`ui/src/app.html`, PWA manifest icons) with
      `mockups/mobile-redesign/assets/icon.svg`-derived assets at the required sizes. Verify: the
      browser tab favicon and the PWA install icon both show the new mark.

## 2. Sign-in gate

- [ ] 2.1 Build the gate component from `mockups/mobile-redesign/signin.html`'s layout (brand
      moment, steps, textarea, submit, error/checking states) as a real Svelte component, wired to
      the real `api.signInWithCookie`/`getAccount` instead of the mockup's fake `setTimeout`.
      Verify: submitting a cookie that fails real validation shows the real error message from the
      backend; submitting one that succeeds transitions to the app.
- [ ] 2.2 Add the root-layout check (design.md Decision 1): call `getAccount()` once at the root
      layout and render only the gate when `!signedIn`, for every route. Verify: opening any route
      URL directly (not just `/`) with no session renders the gate, not that route's content or a
      broken partial render of it.
- [ ] 2.3 Update the cookie-paste steps (both this gate and the existing "Add another account" flow
      in task 3.3) to point at the exact `account_menu` endpoint instead of "find any request."
      Verify: the real app's `ui/src/lib/locales/en.json` `sign_in_cookie_step*` strings and the
      README's "Getting your cookie" section both match the mockup's updated wording.

## 3. Account Profile page

- [ ] 3.1 Build `/profile` from `mockups/mobile-redesign/profile.html`'s layout: current account
      card, switch-account list (reusing `getGoogleAccounts`/`switchGoogleAccount`), sign out
      (`signOut`). Verify: switching accounts from this page actually changes the active session
      (confirm via a subsequent `getAccount()` call reflecting the new account).
- [ ] 3.2 Add the avatar entry point in `HomeHero.svelte`'s header linking to `/profile`, replacing
      the plain history-button-only layout from `mobile-responsive-ui`'s fix. Verify: tapping the
      avatar on Home navigates to `/profile`; the History button remains reachable alongside it.
- [ ] 3.3 Build the "Add another account" flow at a route reachable from Profile, reusing the gate
      component's cookie-paste UI from task 2.1 (same form, different entry/exit: assumes an
      existing session, returns to Profile on success instead of to Home). Verify: completing this
      flow while already signed in adds a new saved account without ending the current session.

## 4. Mobile Settings page

- [ ] 4.1 Build `/settings` from `mockups/mobile-redesign/settings.html`'s grouped-list layout
      (Account / Appearance / Playback / Integrations / About), wiring each row to its real command
      (`getSettings`/`setSetting`, the real theme store for dark mode, etc.) instead of the mockup's
      local-only toggles. Verify: toggling dark mode here persists the same way the existing theme
      toggle does elsewhere in the app (shared state, not a second independent theme flag).
- [ ] 4.2 Link Settings → Downloads and Settings → Local files (the existing `/offline` route) rows.
      Verify: both rows navigate correctly; Local files' row does not regress
      `offline-local-music-pwa`'s existing behavior.

## 5. Offline downloads

- [ ] 5.1 Add `pinned: boolean` to `yt-cache.ts`'s `CacheEntry`, plus `pin(videoId)`,
      `unpin(videoId)`, and `listDownloaded()`, and make `evictIfOverBudget` skip pinned entries.
      Verify: a unit test confirms a pinned entry survives `evictIfOverBudget` even when it is the
      least-recently-accessed entry over budget.
- [ ] 5.2 Add a download action (e.g. on track rows' overflow menu) that calls `pin` after ensuring
      the track is already cached (triggering a download if it isn't). Verify: downloading a track
      that was never played before results in a cached, pinned entry with no playback required
      first.
- [ ] 5.3 Build `/downloads` from `mockups/mobile-redesign/downloads.html`'s layout: storage used
      (sum of pinned entries' `bytes`), downloaded tracks/playlists list, remove action (`unpin`,
      or `forget` if fully removing rather than just unpinning — resolve which during
      implementation against what "remove a download" should mean for a track that's also in the
      recent play-cache). Verify: removing a download frees its reported storage and the track
      requires network to play again afterward.
- [ ] 5.4 Verify offline playback end to end: download a track, disconnect from the network, confirm
      it plays. Verify: this is the capability's actual acceptance bar (`offline-downloads` spec's
      "plays with no network connection" requirement), not just that the Blob exists in IndexedDB.

## 6. Verification

- [ ] 6.1 Walk through all three new specs' scenarios end to end (`web-signin-gate`,
      `account-profile`, `offline-downloads`) in a real browser session against the real backend,
      not the mockups. Verify: every scenario holds.
- [ ] 6.2 Confirm no regression to existing capabilities: anonymous search is deliberately gone
      (expected, per the BREAKING note) — confirm every *other* existing feature (library, playback,
      Listen Together, local files) still works once signed in. Verify: `pnpm check` passes and a
      manual pass through the main nav finds no regression.
