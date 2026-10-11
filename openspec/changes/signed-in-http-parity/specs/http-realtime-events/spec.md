## Purpose

Gives every other capability in this change a way to push live updates to a connected browser
client — replacing the desktop app's ~20 named Tauri events (`now-playing`, `auth-changed`,
`queue-changed`, `lt-state`, …) with one real-time channel a browser can actually subscribe to.

## ADDED Requirements

### Requirement: Subscribe to real-time events
The system SHALL let a client open a single real-time connection over which it receives every
event type it is entitled to (scoped to its own session for signed-in-only events), each
identifiable by a type name matching the equivalent desktop event.

#### Scenario: Receiving an event after a relevant action
- **WHEN** a client is connected and an action occurs that the desktop app would emit an event for
  (e.g. the signed-in account's auth state changes)
- **THEN** the connected client receives an event identifying that type and carrying the same
  information the desktop app's event payload carries

#### Scenario: A signed-in-only event is never sent to another session
- **WHEN** one client's signed-in session changes state (e.g. its queue)
- **THEN** a different, unrelated client's connection never receives that event

### Requirement: Reconnection does not lose session state
The system SHALL allow a client to reconnect the real-time channel (e.g. after a brief network
interruption) and continue receiving events for its existing session without needing to redo
sign-in or any other setup.

#### Scenario: Reconnecting
- **WHEN** a signed-in client's real-time connection drops and it reconnects
- **THEN** it resumes receiving events for the same session, without being signed out
