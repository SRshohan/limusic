## Purpose

Lets a plain HTTP client resolve a YouTube Music browse target — the home feed, an album, an
artist, a playlist, the Moods & Genres tiles, or a browse grid — to its content, anonymously, the
same way `search-http-api` already resolves a text query.

## ADDED Requirements

### Requirement: Get the home feed over HTTP
The system SHALL accept an HTTP request for the home feed and respond with its sections, and
SHALL accept a continuation token from a prior response to fetch the next page of sections.

#### Scenario: The default home feed
- **WHEN** a client requests the home feed with no continuation token
- **THEN** the response is a successful home page containing at least one section

#### Scenario: Continuing the home feed
- **WHEN** a client requests more sections using a continuation token taken from a prior home-feed
  response
- **THEN** the response is a successful page of additional sections

#### Scenario: An invalid or expired continuation token
- **WHEN** a client requests more sections using a token that YouTube no longer accepts
- **THEN** the response is a client or upstream error, not a server crash

### Requirement: Get an album's page over HTTP
The system SHALL accept an HTTP request carrying an album id and respond with that album's title
and track listing.

#### Scenario: A valid album id
- **WHEN** a client requests a real album's id
- **THEN** the response is a successful album page including its title and tracks

#### Scenario: An unknown album id
- **WHEN** a client requests an id that does not resolve to a real album
- **THEN** the response is successful with an empty/null page (YouTube's own `browse` endpoint
  responds `200` with no content for an id it doesn't recognize, the same way it does for other
  not-found browse targets in this capability — not a server crash)

### Requirement: Get an artist's page over HTTP
The system SHALL accept an HTTP request carrying an artist id and respond with that artist's page
content (at least their name and a list of their releases or top songs).

#### Scenario: A valid artist id
- **WHEN** a client requests a real artist's id
- **THEN** the response is a successful artist page

#### Scenario: An unknown artist id
- **WHEN** a client requests an id that does not resolve to a real artist
- **THEN** the response is successful with an empty/null page (YouTube's own `browse` endpoint
  responds `200` with no content for an id it doesn't recognize — not a server crash)

### Requirement: View a playlist's page over HTTP
The system SHALL accept an HTTP request carrying a playlist id and respond with that playlist's
contents for viewing, and SHALL accept a continuation token to fetch further tracks. This is a
read-only view — no write action (reordering, add/remove, sort) is part of this requirement.

#### Scenario: A valid public playlist id
- **WHEN** a client requests a real, public playlist's id
- **THEN** the response is a successful playlist page including its tracks

#### Scenario: Continuing a playlist's track list
- **WHEN** a client requests more tracks using a continuation token taken from a prior response
  for the same playlist
- **THEN** the response is a successful page of additional tracks

#### Scenario: A desktop-local playlist id
- **WHEN** a client requests an id that only exists in the desktop app's own local library (e.g.
  its "On Repeat" playlist or a locally-scanned album/artist id)
- **THEN** the response is a client error, not a crash and not a silently empty or wrong page —
  that id has no meaning without the desktop app's local SQLite library, which this server does
  not have access to

#### Scenario: An unknown playlist id
- **WHEN** a client requests an id that does not resolve to a real playlist
- **THEN** the response is successful with an empty/null page (YouTube's own `browse` endpoint
  responds `200` with no content for an id it doesn't recognize, same as an unknown album or
  artist id — not a server crash)

### Requirement: Get Moods & Genres over HTTP
The system SHALL accept an HTTP request for the Moods & Genres tiles and respond with their
sections, and SHALL accept a request for cover art for a given set of tile parameters.

#### Scenario: Moods & Genres tiles
- **WHEN** a client requests the Moods & Genres tiles
- **THEN** the response is a successful list of mood/genre sections

#### Scenario: Cover art for a tile that has none of its own
- **WHEN** a client requests cover art for a set of mood/genre tile parameters
- **THEN** the response is successful even if one or more tiles have no art available (a tile with
  no art is simply left out of the response, not treated as an error for the whole request)

### Requirement: Get a browse-grid page over HTTP
The system SHALL accept an HTTP request carrying a browse id (and optional params) and respond
with the resulting grid of items (e.g. a "New releases" or genre grid).

#### Scenario: A valid browse id
- **WHEN** a client requests a real browse id
- **THEN** the response is a successful list of items

#### Scenario: An unknown browse id
- **WHEN** a client requests an id that does not resolve to a real browse target
- **THEN** the response is an error, not a server crash

### Requirement: Reachable from a browser running on a different origin
The system SHALL allow a web page served from a different origin to make these browse requests
directly from browser script, without the request being blocked by the browser's own cross-origin
policy — the same cross-origin allowance `search-http-api` already provides for search.

#### Scenario: Cross-origin request from the app's own frontend
- **WHEN** the frontend, served from its own development origin, issues a browse request to this
  server's origin
- **THEN** the browser allows the response to be read by the page's script
