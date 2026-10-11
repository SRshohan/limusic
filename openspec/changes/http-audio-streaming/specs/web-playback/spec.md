## Purpose

Lets a browser client actually play music: a real queue, transport controls, and now-playing state
driven entirely in the browser (no server-pushed playback events for this build — the browser is
the player), reusing the same playback engine the offline/local-music build already uses.

## ADDED Requirements

### Requirement: Play a track and advance a queue
The system SHALL let a client start playback of a chosen track, maintain a queue around it, and
advance to the next queued track automatically when the current one finishes.

#### Scenario: Starting playback
- **WHEN** a client starts playback of a track (from a song row, album, playlist, artist, or radio)
- **THEN** that track begins playing and becomes the current now-playing track

#### Scenario: A track finishes
- **WHEN** the currently playing track reaches its end with another track queued after it
- **THEN** playback advances to that next track automatically

#### Scenario: Queue exhausted
- **WHEN** the currently playing track finishes with nothing queued after it
- **THEN** playback stops cleanly rather than erroring

### Requirement: Transport controls
The system SHALL let a client pause, resume, seek, skip to the next or previous track, and adjust
volume for the current playback session.

#### Scenario: Pause and resume
- **WHEN** a client pauses an active track and later resumes it
- **THEN** playback stops and later continues from the paused position

#### Scenario: Seeking
- **WHEN** a client seeks to a position within the current track
- **THEN** playback continues from that position

### Requirement: Gapless and crossfade transitions
The system SHALL transition between consecutive tracks with no audible gap when crossfade is
disabled, and with an equal-power overlapping fade when a crossfade duration is set.

#### Scenario: Crossfade disabled
- **WHEN** one track ends and the next begins with crossfade set to zero
- **THEN** the next track starts immediately with no audible gap or overlap

#### Scenario: Crossfade enabled
- **WHEN** a crossfade duration is set and a track nears its end
- **THEN** the next track fades in while the current one fades out, overlapping for that duration

### Requirement: Media-session integration
The system SHALL expose current playback (title, artist, artwork, position) and transport actions
(play/pause/seek/next/previous) to the browser's OS-level media controls.

#### Scenario: Backgrounded playback
- **WHEN** a track is playing and the browser tab is not in the foreground
- **THEN** the OS media controls show the current track and respond to play/pause/next/previous

### Requirement: Host playback for a Listen Together room
The system SHALL let a client hosting a Listen Together room broadcast its own play/pause/seek/
track-change actions to the room, when it already supports hosting a room.

#### Scenario: Hosting a room while playing
- **WHEN** a client is the host of a Listen Together room and changes its own playback (plays,
  pauses, seeks, or changes track)
- **THEN** that action is broadcast to the room's other participants
