## Purpose

Guarantees that concurrent signed-in users on the same running server keep independent account
state — session, YouTube login, library, playlists, settings, and watch/play history — with no
user able to see or affect another's, and states the concurrent-user scale this guarantee is
designed and sized for.

## ADDED Requirements

### Requirement: Concurrent sessions act on independent account state
The system SHALL let more than one user be signed in at the same time, each acting against their
own YouTube account's state, with no action taken under one user's session visible or applied to
another user's account.

#### Scenario: Two users signed in at once
- **WHEN** user A and user B each have an established session for their own, different YouTube
  accounts
- **THEN** a request under A's session only ever reads or writes A's account state, and a request
  under B's session only ever reads or writes B's

#### Scenario: One user's action is invisible to another
- **WHEN** user A performs an account-scoped action (e.g. liking a song, creating a playlist,
  changing a setting)
- **THEN** user B's view of their own library, playlists, settings, and history is unaffected and
  shows no trace of user A's action

### Requirement: Account-scoped data persists independently per account
The system SHALL store library, playlist, settings, and watch/play-history data scoped to the
account it belongs to, so that data for one account is never returned, overwritten, or merged as
part of another account's data.

#### Scenario: Two accounts' libraries coexist
- **WHEN** user A and user B each have songs saved to their library
- **THEN** user A's library listing contains only what A saved, and user B's contains only what B
  saved

#### Scenario: Two accounts' playlists coexist
- **WHEN** user A and user B each create a playlist with the same name
- **THEN** both playlists exist independently, each editable and deletable only in its own
  account's context, and neither user sees the other's playlist as their own

### Requirement: Shared, account-agnostic infrastructure is unaffected by concurrency
The system SHALL serve every concurrently signed-in user's requests through the same shared,
non-account-specific stream-resolution infrastructure (signature deciphering, bot-attestation
token minting), without that sharing leaking one account's data into another's results.

#### Scenario: Two users resolve the same track at once
- **WHEN** user A and user B both request playback of the same video around the same time
- **THEN** both requests succeed independently, and neither user's playback is delayed, blocked, or
  altered by the other's concurrent request beyond ordinary shared-resource queuing

### Requirement: A documented concurrent-user capacity
The system's design SHALL state a concrete target for how many users can be signed in
concurrently on one running server, and what resource(s) actually bound that target, so operators
know what scale this guarantee is sized for rather than discovering a limit by failure.

#### Scenario: Operating within the documented target
- **WHEN** the number of concurrently signed-in users is at or below the documented target
- **THEN** every requirement in this capability holds as specified

#### Scenario: Looking up the capacity target
- **WHEN** an operator wants to know how many concurrent users this server is designed to support
- **THEN** the design documentation states a concrete number (or range) and the resource it is
  bounded by
