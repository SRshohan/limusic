## Purpose

Lets a browser client turn a YouTube videoId into a playable audio (or music-video) stream over
plain HTTP, without ever seeing YouTube's own throttled, cipher-locked, CORS-blocked URLs.

## ADDED Requirements

### Requirement: Resolve a videoId to a playable stream
The system SHALL respond, for a given videoId, with the track's playable stream location and its
display metadata (title, artists, duration, thumbnail), without requiring a signed-in session.

#### Scenario: Resolving an ordinary track, signed out
- **WHEN** a client requests resolution for a videoId with no session
- **THEN** the response is successful and includes a stream location and display metadata

#### Scenario: Resolving a track that needs an account
- **WHEN** a client requests resolution for a videoId that YouTube will not serve anonymously
- **THEN** the response is a clear, actionable error distinguishing "sign in to play this" from "this
  track can't be played"

#### Scenario: Resolving an unplayable videoId
- **WHEN** a client requests resolution for a videoId no available path can stream
- **THEN** the response is a clear client error, not a server crash

### Requirement: Stream audio through a throttling-safe proxy
The system SHALL serve a resolved track's audio bytes through a server-side proxy that supports
HTTP range requests, so a browser `<audio>` element can seek and buffer normally.

#### Scenario: Playing a resolved track
- **WHEN** a client requests the stream URL returned by resolution
- **THEN** audio bytes are returned, honoring byte-range requests

#### Scenario: Seeking within a track
- **WHEN** a client requests a byte range past the start of the stream
- **THEN** the response starts at that range, not from the beginning

### Requirement: Stream music video through the same proxy pattern
The system SHALL serve a resolved track's video bytes through an equivalent range-respecting proxy
when the track has an associated music video.

#### Scenario: Playing a track's music video
- **WHEN** a client requests the video stream for a videoId that has one
- **THEN** video bytes are returned, honoring byte-range requests
