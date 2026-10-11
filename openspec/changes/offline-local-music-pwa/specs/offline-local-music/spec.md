## Purpose

Lets a person listen to their own audio files through Limusic with zero network or server
reachability of any kind, by importing and playing them entirely inside the browser.

## ADDED Requirements

### Requirement: Import a folder of local audio files
The system SHALL let the user grant access to a folder on their device and index every supported
audio file found in it (recursively), without uploading any file content anywhere.

#### Scenario: Picking a folder indexes its audio files
- **WHEN** the user picks a folder containing audio files (optionally in subfolders)
- **THEN** every supported audio file in that folder and its subfolders appears in the local
  library, with title/artist/album metadata read from the file's own tags where present

#### Scenario: Unsupported files are ignored
- **WHEN** a picked folder contains non-audio files or audio files in an unsupported format
- **THEN** those files are skipped and do not appear in the library, and the import still
  completes for the supported files

#### Scenario: A file with no readable tags still gets a title
- **WHEN** an imported audio file has no embedded title tag (or its tags fail to parse)
- **THEN** the file still appears in the library, titled from its filename

### Requirement: Import individual files as a fallback
The system SHALL let the user import one or more individual audio files through a standard file
picker on browsers that do not support folder access, and these files SHALL remain playable after
the browser is closed and reopened.

#### Scenario: Importing files on a browser without folder access
- **WHEN** the user's browser does not support picking a folder and they instead select one or
  more audio files through a file picker
- **THEN** those files are indexed into the library and remain playable in a later session without
  being re-selected

### Requirement: Imported sources can be rescanned or removed
The system SHALL let the user re-scan a previously imported folder to pick up added/removed files,
and remove an imported source entirely (dropping its tracks from the library).

#### Scenario: Rescanning picks up new files
- **WHEN** the user adds files to a folder that was previously imported and then rescans that
  source
- **THEN** the newly added files appear in the library without duplicating previously imported
  ones

#### Scenario: Removing a source clears its tracks
- **WHEN** the user removes an imported source
- **THEN** none of that source's tracks appear in the library afterward

### Requirement: Re-granting access when permission has lapsed
The system SHALL detect when the browser no longer has permission to read a previously imported
folder or file and prompt the user to re-grant it, rather than failing silently.

#### Scenario: Playing a track after permission lapsed
- **WHEN** the user tries to play or rescan a track/source whose file-access permission the
  browser has revoked since it was imported
- **THEN** the user is prompted to re-grant access, and playback/rescanning proceeds once granted

### Requirement: Gapless playback within a queue
The system SHALL play a queue of local tracks back-to-back with no audible gap or silence between
tracks by default.

#### Scenario: One track ends and the next begins immediately
- **WHEN** a track in the queue reaches its end and a next track exists in the queue
- **THEN** the next track begins playing with no perceptible silence between them

### Requirement: Configurable crossfade between tracks
The system SHALL let the user set a crossfade duration, and when set to a positive value, overlap
the end of one track with the start of the next using an equal-loudness fade rather than an abrupt
cut or silent gap.

#### Scenario: Crossfade overlaps two tracks
- **WHEN** the user has set a crossfade duration greater than zero and a track nears its end with a
  next track in the queue
- **THEN** the next track audibly fades in while the current one fades out, overlapping by
  approximately the configured duration, and the combined loudness does not noticeably dip or
  spike during the overlap

#### Scenario: Crossfade is capped by short tracks
- **WHEN** the configured crossfade duration is longer than half of the current or next track's
  length
- **THEN** the overlap is shortened so it never exceeds half of either track's duration

### Requirement: Lock-screen and OS media-key controls
The system SHALL expose the currently playing track's title, artist, album art, and transport
controls (play, pause, seek, next, previous) to the operating system, so they work from the
lock screen, a notification, or hardware media keys without the app being in focus.

#### Scenario: Pausing from a hardware media key
- **WHEN** a track is playing and the user presses a hardware/OS pause control outside the app
- **THEN** playback pauses, and the OS control reflects the paused state

#### Scenario: Now-playing info is visible system-wide
- **WHEN** a track is playing
- **THEN** the operating system's now-playing surface (lock screen, notification, etc.) shows that
  track's title and artist

### Requirement: Playback requires no network connectivity
The system SHALL play any previously imported local track, build and browse the local library, and
manage local playlists with no network connection present, after the app itself has been loaded at
least once.

#### Scenario: Playing a local track with no network
- **WHEN** the device has no network connectivity (e.g. airplane mode) and the app was previously
  installed/loaded
- **THEN** the user can browse their local library and play any previously imported track without
  error attributable to the lack of connectivity
