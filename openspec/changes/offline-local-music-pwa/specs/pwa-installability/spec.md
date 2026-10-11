## Purpose

Makes Limusic installable as a standalone app from a browser and ensures the app shell itself
loads instantly and reliably offline, independent of whether any particular feature needs network
access.

## ADDED Requirements

### Requirement: App is installable from a supporting browser
The system SHALL present a valid web app manifest (name, icons, standalone display mode) and
register a service worker, so a supporting browser offers to install it as a standalone app.

#### Scenario: Installing from Chrome/Edge
- **WHEN** the user opens the app in a browser that supports PWA installation
- **THEN** the browser offers an install option, and installing it adds a standalone app with the
  app's name and icon

### Requirement: App shell loads offline after first visit
The system SHALL precache its own application shell (HTML/CSS/JS/icons) so the app opens and its
UI renders with no network connection, after having been loaded at least once while online (or
served locally).

#### Scenario: Reopening with no network
- **WHEN** the user has previously opened the app at least once and then opens it again with no
  network connection
- **THEN** the app's interface loads and renders, rather than showing a browser offline error

### Requirement: User-controlled update activation
The system SHALL detect when a new version of the app has been deployed and prompt the user before
switching to it, rather than silently reloading and discarding in-progress state (e.g. a playing
track).

#### Scenario: A new version is available while a track is playing
- **WHEN** a new version of the app has been deployed and the user has the app open with a track
  playing
- **THEN** the user sees an update-available prompt and playback continues uninterrupted until they
  choose to update

### Requirement: Streamed/media routes are never intercepted by the service worker
The system SHALL scope its offline caching to the application shell and explicitly exclude any
media-streaming or realtime-connection route from interception, so a future network-dependent
playback source is never served stale or broken content by the cache.

#### Scenario: A non-shell network route is requested
- **WHEN** the app requests a resource under a media-streaming or realtime-connection path
- **THEN** that request passes through to the network uninterrupted by the service worker's cache
