## MODIFIED Requirements

### Requirement: Sign in by pasting a session cookie
The system SHALL accept an HTTP request carrying a YouTube session cookie string, validate it
against YouTube, and on success establish a server-held session for that account, returning a
session identifier the client uses for subsequent requests without resending the cookie. A
session established for one account SHALL NOT end, replace, or otherwise affect a different,
already-established session for another account.

#### Scenario: A valid cookie
- **WHEN** a client submits a cookie string that YouTube accepts
- **THEN** the response is successful, a server-held session is established for that account, and
  the client receives a session identifier (e.g. an `HttpOnly` cookie) it can use without resending
  the YouTube cookie itself

#### Scenario: An invalid or expired cookie
- **WHEN** a client submits a cookie string YouTube rejects
- **THEN** the response is a client error, and no session is established

#### Scenario: A second account signs in while the first is still active
- **WHEN** a client establishes a session for account B while a different, already-established
  session for account A exists
- **THEN** both sessions remain independently valid: a request carrying account A's session
  identifier still acts as account A, and a request carrying account B's acts as account B

### Requirement: Sign out
The system SHALL allow a client to end its own session, after which requests that require being
signed in and carry that session's identifier are no longer served as that account. Ending one
session SHALL NOT affect any other established session.

#### Scenario: Signing out
- **WHEN** a client with an established session requests sign-out
- **THEN** the response is successful, and a subsequent request carrying that session's identifier
  that requires being signed in is treated as signed out, not as the previous account

#### Scenario: Signing out does not affect other sessions
- **WHEN** a client with an established session (account A) requests sign-out while a different,
  already-established session for account B exists
- **THEN** account A's session ends, and a subsequent request carrying account B's session
  identifier continues to act as account B
