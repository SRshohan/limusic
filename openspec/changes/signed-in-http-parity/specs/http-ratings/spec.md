## Purpose

Lets a signed-in browser client rate a song, save a song or album to their library, and subscribe
to an artist, over plain HTTP.

## ADDED Requirements

### Requirement: Rate a song
The system SHALL let a signed-in client set a song's rating (e.g. liked, disliked, or neutral).

#### Scenario: Setting a rating
- **WHEN** a signed-in client requests a rating change for a real song
- **THEN** the response is successful and a subsequent view of that song reflects the new rating

### Requirement: Save a song or album to the library
The system SHALL let a signed-in client save or unsave a song, and save or unsave an album, to and
from their library.

#### Scenario: Saving a song
- **WHEN** a signed-in client requests saving a song
- **THEN** the response is successful and the song subsequently appears in their library

#### Scenario: Saving an album
- **WHEN** a signed-in client requests saving or unsaving an album
- **THEN** the response is successful and the album's saved state is reflected on a subsequent view

### Requirement: Subscribe to an artist
The system SHALL let a signed-in client subscribe to or unsubscribe from an artist.

#### Scenario: Subscribing
- **WHEN** a signed-in client requests subscribing to a real artist
- **THEN** the response is successful and a subsequent view of that artist reflects the subscribed
  state
