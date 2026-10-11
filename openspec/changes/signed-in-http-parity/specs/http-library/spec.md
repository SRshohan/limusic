## Purpose

Lets a signed-in browser client view their YouTube Music library — liked songs, saved albums,
saved artists, uploads, watch history, and local play counts — over plain HTTP.

## ADDED Requirements

### Requirement: View the signed-in library
The system SHALL respond with the signed-in account's library contents (liked songs, saved
albums, saved artists, and uploads) on request.

#### Scenario: A signed-in account with library content
- **WHEN** a signed-in client requests their library
- **THEN** the response is a successful list reflecting their liked songs, saved albums, saved
  artists, and uploads

#### Scenario: A signed-in account with an empty library
- **WHEN** a signed-in client with nothing saved requests their library
- **THEN** the response is a successful, empty result — not an error

### Requirement: View watch history
The system SHALL respond with the signed-in account's watch history, grouped the same way YouTube
Music's own history is grouped (by day).

#### Scenario: History requested
- **WHEN** a signed-in client requests their history
- **THEN** the response is a successful list of history groups

### Requirement: View local play counts
The system SHALL respond with per-track play counts recorded for the signed-in account (feeding
features like "On Repeat"), scoped to that server-side session's own recorded plays.

#### Scenario: Play counts requested
- **WHEN** a signed-in client requests their play counts
- **THEN** the response is a successful mapping of track identifiers to play counts, empty if
  nothing has been recorded yet
