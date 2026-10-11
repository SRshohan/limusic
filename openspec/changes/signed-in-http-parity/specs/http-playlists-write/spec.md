## Purpose

Lets a signed-in browser client create, edit, delete, reorder, and change the membership/cover of
a playlist they own, over plain HTTP. Viewing a playlist's contents is `browse-http-api`'s job;
this capability is the write side only.

## ADDED Requirements

### Requirement: Create and delete a playlist
The system SHALL let a signed-in client create a new playlist and delete a playlist they own.

#### Scenario: Creating a playlist
- **WHEN** a signed-in client requests a new playlist with a title
- **THEN** the response is successful and includes the new playlist's id

#### Scenario: Deleting an owned playlist
- **WHEN** a signed-in client requests deletion of a playlist they own
- **THEN** the response is successful and the playlist no longer appears in their library

#### Scenario: Deleting a playlist not owned by the caller
- **WHEN** a signed-in client requests deletion of a playlist they do not own
- **THEN** the response is a client error, and the playlist is not deleted

### Requirement: Edit playlist details and cover
The system SHALL let a signed-in client change an owned playlist's name, description, and
visibility, and set or clear its cover image.

#### Scenario: Editing details
- **WHEN** a signed-in client requests a name/description/visibility change for a playlist they own
- **THEN** the response is successful and the change is reflected on a subsequent view of that
  playlist

#### Scenario: Setting a cover
- **WHEN** a signed-in client requests a cover change for a playlist they own
- **THEN** the response is successful, unless the account lacks whatever verification YouTube
  Music requires for custom playlist art, in which case the response is a client error naming that
  requirement, not a silent no-op

### Requirement: Change playlist membership and order
The system SHALL let a signed-in client add a track to, remove one or more tracks from, and
change the sort order of an owned playlist.

#### Scenario: Adding a track
- **WHEN** a signed-in client requests adding a track to a playlist they own
- **THEN** the response is successful and the track appears in a subsequent view of that playlist

#### Scenario: Adding a track already in the playlist, duplicates not allowed
- **WHEN** a signed-in client requests adding a track already present, without explicitly allowing
  duplicates
- **THEN** the response reports the duplicate rather than silently adding a second copy

#### Scenario: Removing one or many tracks
- **WHEN** a signed-in client requests removing one or more tracks from a playlist they own
- **THEN** the response is successful and those tracks no longer appear in a subsequent view

#### Scenario: Changing sort order
- **WHEN** a signed-in client requests a different sort for a playlist they own
- **THEN** a subsequent view of that playlist reflects the new order
