## Purpose

Lets a browser client host or join a synced-listening room — create/join, approve or reject joins
and track suggestions, kick a participant, and transfer host — over a direct connection to a
user-configured relay (`crates/sync-server`), the same way the desktop app already does. Actual
audio stays per-participant (each still needs their own playback source); this capability is the
room/approval state only. The "system" in the requirements below is that relay, not
`crates/server`/`crates/core` — this capability needs no new backend code, only a browser-side
client speaking the relay's existing protocol (see design.md).

## ADDED Requirements

### Requirement: Create and join a room
The system SHALL let a client create a new room and let another client join one by its code,
subject to host approval.

#### Scenario: Creating a room
- **WHEN** a client requests a new room
- **THEN** the response is successful and includes a join code

#### Scenario: Joining with a valid code, pending approval
- **WHEN** a client requests to join using a real room's code
- **THEN** the request is pending until the host approves or rejects it, and the joining client is
  notified of the outcome

#### Scenario: Joining with an invalid code
- **WHEN** a client requests to join using a code that matches no room
- **THEN** the response is a client error

### Requirement: Host approves or rejects joins and suggestions
The system SHALL let the current host approve or reject a pending join request, and approve or
reject a track suggestion from a participant.

#### Scenario: Approving a join
- **WHEN** the host approves a pending join request
- **THEN** the requesting client is notified it has joined, and other participants are notified of
  the new member

#### Scenario: Rejecting a suggestion
- **WHEN** the host rejects a participant's track suggestion
- **THEN** the suggesting client is notified of the rejection, and the room's queue is unaffected

### Requirement: Host can kick a participant or transfer host
The system SHALL let the current host remove a participant from the room, and transfer the host
role to another participant.

#### Scenario: Kicking a participant
- **WHEN** the host removes a participant
- **THEN** that participant is disconnected from the room and notified why

#### Scenario: Transferring host
- **WHEN** the host transfers the role to another participant
- **THEN** that participant becomes able to approve joins/suggestions and the former host no longer
  can

#### Scenario: Host disconnects without transferring
- **WHEN** the current host disconnects without transferring the role
- **THEN** the host role does not pass to a participant who has also disconnected, matching the
  existing desktop behavior (`fix(sync-server): don't transfer the host role to a disconnected
  user`)

### Requirement: Leaving a room
The system SHALL let a participant (host or not) leave a room they are in.

#### Scenario: A participant leaves
- **WHEN** a non-host participant requests to leave
- **THEN** they are disconnected from the room and other participants are notified
