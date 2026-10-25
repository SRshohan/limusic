## Purpose

Defines how the app's core surfaces — Home, Search, Library, Now Playing/Queue, the mini player
bar, and the bottom tab bar — must behave at phone-width viewports, so the layout reads as
purpose-built for a phone rather than a desktop page compressed into a narrow column.

## ADDED Requirements

### Requirement: No horizontal overflow at phone width
Home, Search, Library, and the Now Playing/Queue view SHALL render with no unintended horizontal
page scrolling and no content clipped by the viewport edge at phone-width viewports (down to
360px wide). Only rows explicitly designed to scroll horizontally (filter chips, card carousels)
may extend past the visible edge.

#### Scenario: Home at a narrow phone width
- **WHEN** Home is viewed at a 360–430px wide viewport
- **THEN** no element is clipped by the screen edge and the page itself does not scroll
  horizontally

#### Scenario: Now Playing/Queue at a narrow phone width
- **WHEN** the Now Playing/Queue view is open at a 360–430px wide viewport
- **THEN** the artwork, tabs, and queue list all fit within the viewport width with no horizontal
  clipping or scrolling

### Requirement: Horizontally scrollable rows signal their own scrollability
Any row that scrolls horizontally (the mood/genre filter chips, card carousels) SHALL visually
indicate that more content exists beyond the visible edge, rather than ending in a hard, unindicated
cutoff.

#### Scenario: Filter chips overflow the viewport
- **WHEN** the mood/genre filter chip row's total width exceeds the viewport width
- **THEN** the chip nearest the trailing edge is partially visible, faded, or otherwise visually
  distinct from a fully-visible chip, so the row is recognizable as scrollable without the user
  swiping first

### Requirement: Card titles have enough width to avoid excessive truncation
Cards in horizontally-scrolling or grid rows (e.g. "Jump back in") SHALL be wide enough at phone
viewport widths that a typical title (up to ~30 characters) displays in full or truncates by no
more than a few trailing characters. Truncation past that point — a title cut down to only its
first word or two — SHALL be addressed by wrapping to a second line or widening the card, not left
as a single aggressively-clipped line.

#### Scenario: Long playlist title on a Home card
- **WHEN** a "Jump back in" (or equivalent) card's title is a typical-length title (up to ~30
  characters) at a phone viewport width
- **THEN** the title is fully visible, or wraps to a second line, rather than being clipped down to
  only its first word or two

### Requirement: Now Playing artwork scales to the viewport
The Now Playing/Queue view's artwork SHALL size relative to the viewport's available space rather
than a fixed dimension, so that the Queue/Lyrics tabs and at least the first queue row are visible
without scrolling past the artwork on a phone-height screen.

#### Scenario: Opening Now Playing on a phone
- **WHEN** the Now Playing/Queue view opens at a phone viewport (width ≤ 430px, height ≤ 900px)
- **THEN** the artwork occupies a size proportional to the viewport, and the Queue/Lyrics tab row
  is visible without first scrolling the page

### Requirement: No dead space between queue content and the mini player
The Now Playing/Queue view SHALL NOT leave a large empty gap between the end of the queue list
content and the mini player bar at the bottom of the screen; the layout SHALL fill available space
proportionally or avoid reserving space the content doesn't use.

#### Scenario: Short queue on a tall phone screen
- **WHEN** the queue has few items and the view is taller than the content needs
- **THEN** the space below the queue list is not a large unused gap disconnected from both the
  queue above it and the mini player below it

### Requirement: Mini player and bottom tab bar are visually distinct with safe-area spacing
On views where both the mini player bar and the bottom tab bar are shown, they SHALL have a visible
boundary (spacing, elevation, border, or similar) between them rather than sitting flush with no
separation, and the bottom-most element SHALL respect the device's safe-area inset so it is not
obscured by a home indicator or similar OS chrome.

#### Scenario: Mini player above the tab bar on Home
- **WHEN** Home is shown on a phone with both the mini player bar and the bottom tab bar visible
- **THEN** a visible boundary separates the two bars, and the tab bar's bottom padding accounts for
  the device's safe-area inset

### Requirement: Header controls have adequate touch spacing
Interactive header elements (account avatar, search field, keyboard-shortcut hint, and similar
controls grouped in a single row) SHALL have enough spacing between them at phone width that
adjacent controls are not visually or functionally crowded together.

#### Scenario: Home header row at phone width
- **WHEN** the Home header row (avatar, search field, shortcut hint) is rendered at a phone-width
  viewport
- **THEN** each control has visible spacing from its neighbors, and no two controls touch or
  overlap
