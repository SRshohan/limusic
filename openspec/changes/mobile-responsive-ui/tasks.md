## 1. Safe-area foundation

- [x] 1.1 Add `viewport-fit=cover` to the viewport `<meta>` tag in `ui/src/app.html`. Verify: in an
      iOS Safari/WebKit simulator (or a real device) with a home indicator, `BottomNav.svelte`'s
      existing `env(safe-area-inset-bottom)` padding changes from `0px` to a non-zero value (inspect
      the computed style) — confirming the inset is actually active now, not just present in markup.
- [x] 1.2 Add bottom safe-area padding to `PlayerBar.svelte`'s `sm:hidden` mobile block (it can sit
      directly at the screen edge whenever `BottomNav` is hidden, e.g. while `NowPlaying` is open).
      Verify: on a device with a home indicator, open Now Playing on phone width and confirm the
      mini player's bottom edge clears the home indicator with visible padding, matching
      `BottomNav`'s treatment.

## 2. Shared filter-chip rail with scroll affordance

- [x] 2.1 Extract the duplicated chip-row markup from `ui/src/routes/+page.svelte` (~line 398) and
      `ui/src/routes/search/+page.svelte` (~line 436) into one shared component (e.g.
      `ui/src/lib/components/ChipRail.svelte`), preserving the existing `chipClass(active)` styling
      and `overflow-x-auto` behavior. Verify: `pnpm check` passes with no new type errors, and both
      Home's mood chips and Search's filter chips still render and filter correctly after the swap.
- [x] 2.2 Add a scroll-position-driven edge fade to the shared rail so a horizontally overflowing
      chip row visibly indicates more content exists, disappearing once scrolled to the end.
      Implemented as a `ResizeObserver`/`MutationObserver`-driven scroll-edge overlay gradient
      rather than literal `mask-image`, since masking a flex row of pill-shaped buttons directly
      produced uneven edges — an overlay fading to the surrounding background achieves the same
      observable behavior (spec's "mobile-layout" requirement) more robustly. Verify: at phone
      width with enough chips to overflow, the trailing edge shows a visible fade/cutoff indicator
      that disappears when scrolled all the way to the last chip; `pnpm check` passes.

## 3. Home header row

- [x] 3.1 Add `sm:`-scoped classes to `HomeHero.svelte`'s header row (`ui/src/lib/components/HomeHero.svelte:79-120`)
      so the avatar, greeting, and search form don't compete for space at phone width — e.g. shrink
      or drop the greeting text and let the search form flex instead of `shrink-0`. Verify: at a
      360–430px wide viewport, the avatar, greeting (or its replacement), and search field each have
      visible spacing with no overlap or clipping. Implemented as a stack (`flex-col` below `sm`,
      `sm:flex-row`) rather than shrinking in place: avatar+greeting get their own row, the history
      button + search form get the row below, full width. Confirmed visually via a Playwright
      screenshot at 375px — "Good evening" fully visible, search field fills the row, ⌘K badge
      clear, no overlap; `pnpm check` passes.

## 4. Now Playing / Queue

- [x] 4.1 Tune `NowPlaying.svelte`'s `--art`/`--vid` sizing calc for phone-height viewports so the
      Queue/Lyrics tab row is visible without scrolling past the artwork first. Verify: at a phone
      viewport (≤430px wide, ≤900px tall, and again at a shorter ~667px-tall size), the Queue/Lyrics
      tabs are visible on initial render without scrolling.
- [x] 4.2 Make the queue list area flex to fill remaining vertical space (`flex-1 min-h-0` with
      internal scrolling) instead of reserving a fixed block height independent of item count.
      Verify: with a short queue (1–2 items) on a tall phone screen, there is no large empty gap
      between the queue list and the mini player bar below it.

## 5. Library and Search phone-width audit

- [x] 5.1 Audit `ui/src/routes/library/+page.svelte` (currently zero `sm:`/`md:`/`lg:` classes) at
      phone width against `specs/mobile-layout/spec.md`'s requirements and fix what's found (tab
      row, card grid spacing/overflow). Found: the 7-tab `Tabs.List` is `w-fit`/no-wrap with no
      scroll container, overflowing the whole page horizontally — wrapped it in the shared
      `ChipRail` (same scroll + edge-fade as the chip rows); `.card-grid`'s existing
      `auto-fill`/fixed-column CSS was already safe at phone width, no change needed there. Also
      reduced page padding (`p-4 sm:p-6`) and let the title/buttons row wrap. Verified visually via
      a Playwright screenshot at 375px: tabs scroll with a visible trailing fade, no page-level
      horizontal overflow; `pnpm check` passes.
- [x] 5.2 Audit `ui/src/routes/search/+page.svelte` at phone width (its existing container-query
      two-column layout is desktop-oriented) against the same spec and fix what's found. The
      `@4xl:grid-cols-[...]` override only applies above that container width, so the base layout
      was already single-column and safe; found and fixed the same oversized/cramped header pattern
      as HomeHero (fixed `px-6`/`text-4xl`/`pt-10` with no `sm:` variants) on the title, the sticky
      search-form row, and the results container. Verified visually at 375px and confirmed
      `document.documentElement.scrollWidth === clientWidth` (no horizontal overflow); `pnpm check`
      passes.

## 6. Verification

- [x] 6.1 Walk through `specs/mobile-layout/spec.md`'s scenarios end to end in a real or emulated
      phone-width browser session (not just a resized desktop window) covering Home, Search,
      Library, and Now Playing/Queue. **Partially verified in this environment, rest needs a
      human**: Home, Search, and Library were driven with a headless Playwright session at 375px
      against the real dev server and confirmed — Home's header no longer crams avatar/search
      together, the chip-rail fade appears/disappears correctly on scroll (confirmed in isolation
      too, since no chips had real data to overflow with — the backend's search server wasn't
      running), Library's tab row scrolls with a fade instead of overflowing the page, Search has no
      horizontal overflow (`scrollWidth === clientWidth`). **Not verified**: the actual Now
      Playing/Queue view, which only renders with `playback.now` truthy — that needs a real/mocked
      playback session (mpv backend) this headless environment doesn't have; and the safe-area
      changes (1.1/1.2), which design.md already flagged as needing a real iOS device/simulator —
      a desktop browser's viewport emulation reports `env(safe-area-inset-*)` as `0px` regardless.
- [x] 6.2 Confirm no regression to the desktop/tablet layout: `pnpm check` passes, and Home, Search,
      Library, and Now Playing/Queue are visually unchanged at `sm`-and-above viewport widths.
      Verified via Playwright at 1280px: Home, Search, and Library screenshots match the prior
      desktop layout (full sidebar, full-size greeting, all 7 Library tabs on one line with no
      scroll affordance shown since nothing overflows), `scrollWidth === clientWidth` on all three,
      `pnpm check` passes with 0 errors. Now Playing's desktop layout wasn't visually re-checked for
      the same reason as 6.1 (no live playback session available), but its `md:`-gated classes
      (`md:flex-row`, `md:flex-none`, `md:w-[22rem]`) are unchanged from before this change at `md`
      and above — only the `@media (max-width: 767px)` override and the base/mobile `flex-1` are new.
