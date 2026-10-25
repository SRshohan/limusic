## Purpose

Defines explicit, user-controlled offline downloads of YouTube tracks — distinct from the existing
silent, evictable play-cache (`yt-cache.ts`): a download the user asks for must still be there when
they go looking for it, not quietly evicted to make room for something else.

## ADDED Requirements

### Requirement: Downloading a track persists it beyond the existing play-cache's eviction
A track the user explicitly downloads SHALL remain available offline regardless of the existing
play-cache's LRU eviction, until the user removes it.

#### Scenario: The play-cache fills up
- **WHEN** the existing play-cache (`yt-cache.ts`) reaches its size cap and evicts least-recently-
  played tracks
- **THEN** no explicitly-downloaded track is evicted as part of that process

### Requirement: Downloaded content plays with no network connection
A downloaded track SHALL play back successfully with no network connection available.

#### Scenario: Playing a downloaded track offline
- **WHEN** the device has no network connection and the user plays a previously-downloaded track
- **THEN** playback starts and completes normally

### Requirement: Storage usage and downloaded items are visible to the user
The Downloads page SHALL show total storage used by downloads and list every downloaded track and
playlist/album.

#### Scenario: Viewing the Downloads page
- **WHEN** the user opens the Downloads page
- **THEN** total storage used and a list of every downloaded item are displayed

### Requirement: A download can be removed
The user SHALL be able to remove an individual downloaded track (or an entire downloaded playlist/
album) from the Downloads page, freeing its storage.

#### Scenario: Removing a downloaded track
- **WHEN** the user removes a downloaded track from the Downloads page
- **THEN** it no longer appears in the downloaded list, its storage is freed, and playing it again
  requires a network connection like any other track

### Requirement: Downloads are audio only
This capability SHALL cover audio downloads only; music-video downloads are out of scope.

#### Scenario: A track with an available music video
- **WHEN** the user downloads a track that also has a music video
- **THEN** only the audio is downloaded for offline playback
