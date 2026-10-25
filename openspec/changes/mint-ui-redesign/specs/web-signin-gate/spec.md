## Purpose

Defines the web/PWA build's sign-in gate: the screen shown in place of the whole app when there is
no valid signed-in session, and what it must guarantee before letting the user past it.

## ADDED Requirements

### Requirement: No app content renders without a valid session
The web/PWA build SHALL render only the sign-in gate — no Home, Search, Library, or any other
route's content — whenever there is no valid signed-in session for the current browser.

#### Scenario: Opening the web app with no prior session
- **WHEN** the web app is opened in a browser with no existing session
- **THEN** the sign-in gate is the only thing rendered; no search, browse, or playback UI is
  reachable

#### Scenario: A previously valid session has expired
- **WHEN** the web app is opened and the stored session is no longer valid
- **THEN** the sign-in gate renders in place of whatever route was requested, not a partially
  broken version of that route

### Requirement: A submitted cookie is validated against YouTube before being accepted
The gate SHALL submit the pasted cookie to the existing sign-in validation path (a real check
against YouTube, not a local format check alone) and SHALL NOT treat submission as successful
until that check confirms the cookie authenticates.

#### Scenario: Pasting a cookie that YouTube rejects
- **WHEN** the user submits a cookie that does not authenticate with YouTube
- **THEN** the gate shows the specific failure reason returned by the validation check and remains
  on the gate — it does not advance to the app

#### Scenario: Pasting a cookie that YouTube accepts
- **WHEN** the user submits a cookie that successfully authenticates with YouTube
- **THEN** the gate hands off to the app's normal entry point (Home) with a valid session now
  established

### Requirement: The gate communicates that validation is in progress
Because validating a cookie is a real network round trip, not an instant check, the gate SHALL show
a distinct in-progress state between submission and the result, rather than appearing to hang or
instantly resolving.

#### Scenario: Submitting a cookie
- **WHEN** the user submits a cookie
- **THEN** the submit control reflects an in-progress state until the validation result is known,
  and does not accept a second submission while one is already in flight
