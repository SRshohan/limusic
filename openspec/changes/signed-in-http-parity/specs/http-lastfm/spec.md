## Purpose

Lets a signed-in browser client connect, check the status of, and disconnect Last.fm scrobbling,
over plain HTTP.

## ADDED Requirements

### Requirement: Connect Last.fm
The system SHALL let a signed-in client start a Last.fm connection flow and report once it
completes.

#### Scenario: Starting a connection
- **WHEN** a signed-in client requests a Last.fm connection
- **THEN** the response guides the client to complete authorization, and a subsequent status
  request reflects "connected" once the account approves it

#### Scenario: No Last.fm credentials configured on the server
- **WHEN** a signed-in client requests a Last.fm connection on a server with no Last.fm API
  credentials configured
- **THEN** the response is a clear error stating Last.fm is not configured, not a crash or a
  silent no-op

### Requirement: Check Last.fm status
The system SHALL respond with whether the signed-in account currently has Last.fm connected.

#### Scenario: Checking status
- **WHEN** a signed-in client requests Last.fm status
- **THEN** the response reflects the current connected/disconnected state accurately

### Requirement: Disconnect Last.fm
The system SHALL let a signed-in client disconnect Last.fm.

#### Scenario: Disconnecting
- **WHEN** a signed-in, Last.fm-connected client requests disconnection
- **THEN** the response is successful and a subsequent status request reflects "disconnected"
