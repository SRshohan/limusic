# SRMusic mobile redesign mockups

Static HTML/CSS/JS, no build step — open any `.html` file directly in a browser to test it.
These are **not** part of the SvelteKit build (`ui/`); they're a sandbox for trying out the
mint-themed mobile redesign before porting anything into the real components. Renamed from
"Limusic" to "SRMusic" per direction — the real app's own name is unchanged for now.

## Files

- `tokens.css` — the design tokens (mint light mode, mint-tinted near-black dark mode) and all
  shared component styles (chip rail, cards, track rows, mini player, tab bar, settings rows).
- `common.js` — the dark/light toggle (persisted in `localStorage`) and the scroll-rail edge-fade
  behavior, shared by every page.
- `assets/icon.svg` — the app icon: equalizer bars in an asymmetric rise-and-fall (a quiet nod to
  the S/R initials without turning the mark into illegible lettering), mint gradient, superellipse
  container. Built from scratch after a quick survey of current icon-design trends (one
  recognizable shape, one accent/gradient, legible at thumbnail size) — not copied from any
  specific existing app or kit.
- `signin.html` — the hard gate shown instead of the whole app when there's no signed-in account
  at all: no back button, no tab bar, no mini player (nothing to go back to, nothing playable yet).
  Demonstrates all three real states the actual cookie round-trip has — idle, "Checking with
  YouTube…", and either success (redirects to Home) or a real failure message matching
  `state.rs`'s own error strings (`"Sign-in didn't complete — try signing in again."`, etc.) — not
  just the instant happy path.
- `home.html`, `search.html`, `library.html`, `now-playing.html` — one page per main-nav screen,
  linked via the bottom tab bar and the mini player (tap it to open Now Playing). `search.html`'s
  empty state shows "Recent" search pills (matches the real app's `personal.searches` section) —
  tap one to resubmit, remove one with its ✕, or "Clear recent"; typing anything switches to the
  results view and back.
- `profile.html` — current account, switch-account list, "Add another account" (→ `cookie.html`),
  sign out. Opened via the avatar in Home's header, not a bottom-nav tab.
- `settings.html` — a mobile-native grouped list (Account / Appearance / Playback / Integrations /
  About), not a port of the desktop app's tabbed Settings dialog. Has separate rows for Downloads
  and Local files — see below, they're different features.
- `cookie.html` — "Add another account" from Profile (assumes you're already signed in with at
  least one account) — the web build's cookie-paste sign-in, pulled out of the cramped
  dropdown-menu version it lives in today (`AccountMenu.svelte`) into its own full page with room
  to breathe. Copy (the 4 numbered steps, the security note) matches the real app's existing
  strings. `signin.html` is the same form reframed as the app's mandatory entry gate instead.
- `downloads.html` — **YouTube tracks saved for offline listening** (storage usage, downloaded
  playlists/songs, a remove action, an in-progress download row). This is a new feature, not yet
  built — the underlying mechanism already exists as an *implicit* play-cache
  (`ui/src/lib/yt-cache.ts` downloads every played track's actual audio bytes in the background,
  1 GiB cap, LRU-evicted); what doesn't exist yet is a `pinned` flag so a download survives
  eviction, plus this UI. See the session notes below for the full feasibility answer.
- `local-files.html` — **your own imported files** (was briefly `offline.html` — renamed after the
  first pass confused this with `downloads.html`'s YouTube-sourced tracks, a different feature).
  Mirrors the real `/offline` route's structure (import card, artist/album filter chips, song list
  with Play all/Shuffle, local playlists, its own transport bar — intentionally separate from the
  main mini-player in the real app, since this playback never touches the server). Linked from
  Settings → Playback → "Local files".

## How to test

Open `home.html` in a browser at a phone-width viewport (375–430px — use your browser's device
toolbar, or just narrow the window) and click through:

- Bottom tab bar → switches between Home / Search / Library.
- Search with an empty field → "Recent" pills; type anything → switches to results; clear the
  field → back to "Recent".
- Tap the mini player → opens Now Playing.
- Now Playing's Queue / Lyrics pills → actually switch panels (no page reload).
- Tap the avatar on Home → Profile → Settings → "Downloads" / "Local files" / "Add another
  account" (→ Cookie page), and back.
- `signin.html` directly (simulating a cold, signed-out open): paste anything without `SAPISID` in
  it → a real error message, stays put; paste something with `SAPISID` in it → a "Checking with
  YouTube…" state, then redirects to Home.
- The ☀️/🌙 button (top-right, fixed) *or* Settings' own "Dark mode" row → toggles dark mode,
  persisted across every page via `localStorage`.
- Any row of chips/tabs wider than the screen (Home's mood chips, Library's 7 tabs) → scrolls,
  with a fade on whichever edge still has more to scroll to (matches the real app's
  `ChipRail.svelte`).

## Color tokens

Defined once in `tokens.css`'s `:root` and `:root[data-theme="dark"]` blocks. If these get
approved, the same values should replace the relevant entries in the real app's
`ui/src/routes/layout.css` `@theme` tokens — don't hand-copy per-component, copy the token values
so both stay driven from one source.

## Status

Several rounds in now, built from direction given in conversation plus research into the real
codebase for each new ask (grounded in `yt-cache.ts`, `state.rs`'s `sign_in`, the real
`ChipRail.svelte`/`AccountMenu.svelte`/`/offline` route) rather than guessed from scratch each
time. Not yet wired into the real SvelteKit app — still a sandbox for review.
