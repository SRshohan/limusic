## Context

See proposal.md - Why/What Changes for motivation and scope. This section only covers what the
current implementation already does, since the approach below is about finishing and correcting
that, not introducing new infrastructure.

- **Styling**: Tailwind v4 (`@tailwindcss/vite`, config-less — tokens live in `@theme`-style
  `:root` rules in `ui/src/routes/layout.css`). Breakpoints in use are Tailwind's defaults
  (`sm`=640px, `md`=768px, etc.); `ui/src/routes/search/+page.svelte` additionally uses Tailwind
  v4 container queries (`@container`/`@4xl:`). No custom breakpoint tokens exist and none are
  needed — the gap is applying the existing ones consistently, not inventing new ones.
- **Mobile convention already in place**: `sm:hidden`/`sm:flex`/`sm:block` pairs, not a JS
  `isMobile` store. `Sidebar.svelte` (desktop) is `hidden sm:flex`; `BottomNav.svelte` (phone tab
  bar) is `flex sm:hidden`; `PlayerBar.svelte` already has a separate `sm:hidden` compact mobile
  block distinct from its full desktop layout.
- **This is not a greenfield pass.** `PlayerBar.svelte`, `BottomNav.svelte`, `NowPlaying.svelte`,
  and `Sidebar.svelte` already carry `#mobile-friendly audit` comments from a prior partial effort.
  `NowPlaying.svelte`'s artwork is already sized via CSS custom properties
  (`--art`/`--vid`, `calc(min(100%, 100vh - 11rem) * 0.75)`) and already stacks
  (`flex-col md:flex-row`) instead of the side-by-side desktop layout — the screenshot shows that
  calc is still too generous at phone height, not that the mechanism is missing.
- **Not yet touched at all**: `HomeHero.svelte` (the avatar/greeting/search header row at the top
  of Home — `ui/src/lib/components/HomeHero.svelte:79-120`) has zero responsive classes: a fixed
  `text-4xl` greeting, a fixed `3rem` avatar, and a `shrink-0` search form competing for the same
  row with nothing allowed to shrink or stack. `ui/src/routes/library/+page.svelte` has zero
  `sm:`/`md:`/`lg:` classes anywhere.
- **Filter chip rows** (Home's mood chips and Search's filter chips) are duplicated inline markup
  in `+page.svelte` (~line 398) and `search/+page.svelte` (~line 436), both using the same
  `overflow-x-auto` + shared `.rail` class (`ui/src/routes/layout.css` ~line 1060). `.rail` today
  only styles the WebKit scrollbar thumb — it adds no edge fade, snap points, or other indicator
  that the row scrolls.
- **Safe-area**: exactly one `env(safe-area-inset-bottom)` exists in the whole app
  (`BottomNav.svelte:25`). `ui/src/app.html`'s viewport meta tag has no `viewport-fit=cover`, which
  on iOS Safari/WebKit means `env(safe-area-inset-*)` resolves to `0px` regardless of device — so
  that one existing call is very likely a no-op today. `PlayerBar.svelte` (the mini player, which
  sits directly at the screen's bottom edge whenever `BottomNav` is hidden — e.g. while
  `NowPlaying` is open, since `BottomNav` hides itself under that condition) has no safe-area
  handling at all.

## Goals / Non-Goals

**Goals:**
- Fix the specific defects the two screenshots show, using the project's existing responsive
  mechanism (`sm:`/`md:` utility pairs, Tailwind container queries where already adopted) rather
  than introducing a new one.
- Make `env(safe-area-inset-*)` actually take effect (fix the viewport meta tag) and extend it to
  every bar that can sit flush against a screen edge, not just `BottomNav`.
- Extend the same breakpoint audit to Search and Library (per proposal.md), since Library
  currently has no responsive treatment at all and is as likely to have problems as Home did.

**Non-Goals:**
- No new breakpoint system, CSS-in-JS, or viewport-tracking JS store — the `sm:`/`md:` utility
  convention already in use stays the mechanism.
- No visual redesign of desktop/tablet layouts; changes are scoped to behavior at `sm`-and-below
  (and the `NowPlaying` `md` stacking point, which is already phone-oriented).
- No change to `TheaterMode.svelte` (fullscreen desktop-only feature) or `MiniPlayer.svelte` (the
  separate undecorated Tauri mini-player *window*, unrelated to in-app phone layout despite the
  similar name).
- No change to data flow, commands, or the `api.ts`/`api.tauri.ts`/`api.web.ts` split.

## Decisions

1. **Extract the duplicated filter-chip rail into one shared component** (e.g.
   `ui/src/lib/components/ChipRail.svelte`) instead of fixing the scroll-affordance in both
   `+page.svelte` and `search/+page.svelte` copies separately. Add the overflow indicator as a CSS
   `mask-image` edge fade on the rail container (driven by scroll position so it disappears once
   scrolled to the end) rather than a JS-computed gradient — it's the lowest-overhead approach that
   works with the existing `overflow-x-auto` markup and needs no new state.
   *Alternative considered*: leave the two copies in place and patch each independently. Rejected —
   the duplication is already a maintenance smell the screenshot's bug exposed (fixing one and not
   the other is exactly how this kind of drift happens), and a shared component is a small lift
   given the markup is already nearly identical.

2. **Tune `NowPlaying.svelte`'s existing `--art`/`--vid` calc** rather than replacing the sizing
   mechanism. The stacking behavior (`flex-col md:flex-row`) and CSS-custom-property approach are
   sound and already phone-aware; the fix is a smaller multiplier/cap for narrow+short viewports
   (e.g. accounting for the mini player and tab/safe-area space actually reserved below it) so the
   Queue/Lyrics tabs and first queue row are reachable without scrolling past the artwork first.
   Separately, make the queue list area flex to fill remaining height (`flex-1 min-h-0` with
   internal scrolling) instead of implicitly reserving a block height independent of how many items
   are in the queue — that's the dead-space gap in the screenshot.

3. **Rework `HomeHero.svelte`'s header row with `sm:`-scoped classes**, not a new component: shrink
   or stack the greeting/avatar against the search control below `sm` (e.g. drop the greeting to a
   smaller size or hide it in favor of just the avatar, let the search `<form>` take the remaining
   width instead of `shrink-0` competing with a fixed-width sibling). Kept in the same file because
   the component is small and the fix is purely about which classes apply at which width, not new
   structure.

4. **Fix the safe-area inset at its root cause**: add `viewport-fit=cover` to the viewport meta tag
   in `ui/src/app.html` once, so every existing and new `env(safe-area-inset-*)` call in the app
   (today just `BottomNav`, soon also `PlayerBar`'s mobile block) actually resolves to a non-zero
   value on devices that have one. Add bottom safe-area padding to `PlayerBar.svelte`'s `sm:hidden`
   mobile block specifically for the case where it renders without `BottomNav` beneath it.

5. **Audit Library and Search against the same requirements** (`openspec/specs/mobile-layout`)
   using the same `sm:`/`md:` utility pattern already standard in the codebase, fixing whatever
   phone-width issues are found (Library in particular has none of this today). Scope this to
   layout/spacing/overflow only — no content or feature changes on those pages.

## Risks / Trade-offs

- [Shared `ChipRail.svelte` extraction touches two pages at once] → Low risk: the two existing
  inline blocks are already near-identical (`overflow-x-auto` + shared `.rail` class + a
  `chipClass(active)` helper), so the extraction is close to a pure move, not a rewrite.
- [`viewport-fit=cover` changes how the whole page's safe-area/viewport behaves, not just the one
  bar being fixed] → Mitigation: this is a one-line, additive meta tag change with no other
  existing `env()` consumers to regress (there's exactly one today), and the explicit goal is to
  activate inert code, not introduce new sizing bugs — verify on an iOS device/simulator since the
  effect can't be seen in a desktop browser dev-tools device emulator alone.
- [Tuning `NowPlaying`'s artwork calc for very short phone viewports could under-size it on taller
  phones where the original calc was fine] → Mitigation: keep the change scoped to the calc's
  inputs (cap/multiplier), verified against both a short phone (e.g. SE-class ~667px tall) and a
  tall phone (e.g. ~900px tall) rather than one target size.
- [Library's total absence of responsive classes means its audit scope is less predictable up
  front than Home's, which had a concrete screenshot] → Mitigation: apply the same requirements
  from `specs/mobile-layout/spec.md` as the acceptance bar, and treat any additional issues found
  as in-scope fixes rather than new proposals, per proposal.md's "audit ... and fix what's found."

## Migration Plan

Pure frontend UI change, no data migration. Ship as a normal `ui/` change; no feature flag needed
since every fix is a layout/CSS correction at an existing breakpoint, not new behavior to gate.
Rollback is a normal revert of the commit(s) if a regression surfaces.
