## Purpose

Lets a plain HTTP client resolve a text query to a list of matching songs from YouTube Music,
proving the backend can be reached without Tauri's IPC — the first slice of the self-hosted
server the rest of the online feature builds on.

## ADDED Requirements

### Requirement: Search songs over HTTP
The system SHALL accept an HTTP request carrying a text query and respond with a list of matching
songs, each including at least a title and an identifier usable to play that song, without
requiring the caller to be signed in.

#### Scenario: A query with results
- **WHEN** a client sends a search request with a query that matches real songs (e.g. a well-known
  artist or track name)
- **THEN** the response is a successful list of songs, each with a title and a playable identifier

#### Scenario: A query with no results
- **WHEN** a client sends a search request with a query that matches nothing
- **THEN** the response is a successful, empty list — not an error

#### Scenario: A missing or empty query
- **WHEN** a client sends a search request with no query or an empty one
- **THEN** the response is a client error (not a server crash or an empty-success list treated as
  a valid search)

### Requirement: Reachable from a browser running on a different origin
The system SHALL allow a web page served from a different origin (e.g. a local development server)
to make this search request directly from browser script, without the request being blocked by the
browser's own cross-origin policy.

#### Scenario: Cross-origin request from the app's own frontend
- **WHEN** the frontend, served from its own development origin, issues a search request to this
  server's origin
- **THEN** the browser allows the response to be read by the page's script
