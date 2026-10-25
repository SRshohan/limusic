## Why

The app's core surfaces (Home, Search, Library, Now Playing/Queue, the mini player bar, the bottom
tab bar) were built desktop-first and don't adapt to phone-width viewports — they look like a
desktop page squeezed into a narrow column rather than a layout designed for it. Two screenshots
from the live app at phone width make the problems concrete:

- **Now Playing / Queue**: the album artwork is sized for a much wider viewport, pushing the
  Queue/Lyrics tabs, the "Back to …" link, and the queue list down the page; there's a large dead
  gap between the queue list and the mini player bar; the mini player bar itself sits with almost
  no padding against the screen edge.
- **Home**: the avatar, search field, and ⌘K hint are crammed into one row with no breathing room;
  the mood/genre filter chips overflow off the right edge with no scroll affordance (no partial
  next chip, no fade, no scrollbar) so it isn't obvious they scroll at all; "Jump back in" card
  titles truncate mid-word ("Acoustic Covers of Love Son…"); the mini player bar and the bottom tab
  bar are stacked with almost no visual separation, so the boundary between "now playing" and
  "navigation" is unclear.

This matters now because the PWA/web vertical slice work (`http-search-vertical-slice`,
`offline-local-music-pwa`) is making the browser/mobile path a first-class way to use the app, not
just the Tauri desktop shell — a phone-width session is no longer an edge case.

## What Changes

- Rework the Now Playing/Queue view's phone-width layout: scale the artwork to the viewport instead
  of a fixed desktop size, remove the dead space between the queue list and the mini player, and
  give the mini player proper safe-area padding at the bottom edge.
- Rework the Home page's phone-width layout: give the header row (avatar/search/⌘K hint) enough
  spacing to not feel crammed, make the filter-chip row's horizontal scrollability visible (partial
  next chip and/or edge fade, not a hard cutoff), and stop card titles truncating mid-word (wrap,
  reflow, or a wider card — not a silently-cut string).
- Give the mini player bar and the bottom tab bar a visually distinct boundary (spacing, elevation,
  or a border) instead of sitting flush against each other, with correct safe-area spacing on
  devices with a home indicator.
- Audit the same phone-width breakpoint across Search and Library for the same class of issues
  (cramped spacing, overflow, truncation) and fix what's found, even though no screenshot was taken
  of those two pages specifically.
- Out of scope: no new features, no changes to the desktop/tablet layout's visual design, no
  changes to the underlying Tauri vs. web command split (`api.ts`/`api.tauri.ts`/`api.web.ts`).

## Capabilities

### New Capabilities
- `mobile-layout`: responsive layout requirements for the app's core surfaces (Home, Search,
  Library, Now Playing/Queue, mini player, bottom tab bar) at phone-width viewports — spacing,
  overflow handling, truncation, and safe-area behavior.

### Modified Capabilities
(none — no existing capability's spec describes layout/responsiveness today)

## Impact

- Affected code: SvelteKit components under `ui/src/lib/components/` and `ui/src/routes/` for
  Home, Search, Library, Now Playing/Queue, the mini player, and the bottom tab bar, plus their
  shared CSS/Tailwind breakpoints.
- No backend, API, or data-model changes — this is purely the presentation layer.
- No change to `ui/src/lib/api.ts`/`api.tauri.ts`/`api.web.ts` or any command surface.
