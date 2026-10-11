## Purpose

Lets a browser client retrieve diagnostics text and a summary, and save diagnostics to a file —
via the browser's own file-save mechanism, since a browser has no server-side filesystem path to
write to the way the desktop app does.

## ADDED Requirements

### Requirement: Retrieve diagnostics
The system SHALL respond with diagnostics text and a shorter summary on request.

#### Scenario: Requesting diagnostics
- **WHEN** a client requests diagnostics
- **THEN** the response is successful and includes diagnostics text

#### Scenario: Requesting a diagnostics summary
- **WHEN** a client requests the diagnostics summary
- **THEN** the response is successful and includes a shorter summary, not the full text

### Requirement: Save diagnostics to a file the client controls
The system SHALL provide diagnostics in a form the browser can save to a file the user chooses,
rather than writing to a path on the server.

#### Scenario: Saving diagnostics
- **WHEN** a client requests to save diagnostics
- **THEN** the response provides the diagnostics content such that the browser's own save/download
  mechanism can write it to a file the user picks
