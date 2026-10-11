## Purpose

Lets a browser client view, add to, and remove from the blocked-artists list, over plain HTTP.
Anonymous: this is a local blocklist with no account concept at all (confirmed against the
original Tauri commands, which have no sign-in guard either), not a per-account setting.

## ADDED Requirements

### Requirement: View blocked artists
The system SHALL respond with the current list of blocked artists.

#### Scenario: Listing blocked artists
- **WHEN** a client requests the blocked-artists list
- **THEN** the response is a successful list, empty if nothing is blocked

### Requirement: Block and unblock an artist
The system SHALL let a client add an artist (by id, or by name when no id is available) to the
blocked list, and remove one.

#### Scenario: Blocking an artist
- **WHEN** a client requests blocking an artist
- **THEN** the response is a successful, updated blocked-artists list including that artist

#### Scenario: Unblocking an artist
- **WHEN** a client requests unblocking a previously blocked artist
- **THEN** the response is a successful, updated blocked-artists list no longer including that
  artist
