## Purpose

Lets a browser client sign in to a YouTube Music account, see which identity is active, switch
between saved accounts, and sign out — over plain HTTP, with the server holding the session.

## ADDED Requirements

### Requirement: Sign in by pasting a session cookie
The system SHALL accept an HTTP request carrying a YouTube session cookie string, validate it
against YouTube, and on success establish a server-held session for that account, returning a
session identifier the client uses for subsequent requests without resending the cookie.

#### Scenario: A valid cookie
- **WHEN** a client submits a cookie string that YouTube accepts
- **THEN** the response is successful, a server-held session is established for that account, and
  the client receives a session identifier (e.g. an `HttpOnly` cookie) it can use without resending
  the YouTube cookie itself

#### Scenario: An invalid or expired cookie
- **WHEN** a client submits a cookie string YouTube rejects
- **THEN** the response is a client error, and no session is established

### Requirement: See and switch between signed-in identities
The system SHALL report the currently active account identity, and, for an account with multiple
YouTube channel identities, SHALL allow switching the active identity for the current session.

#### Scenario: Getting the active account
- **WHEN** a client with an established session requests the current account
- **THEN** the response includes that account's identity

#### Scenario: Switching identity
- **WHEN** a client with an established session that has more than one identity requests a switch
  to a different, valid identity
- **THEN** the response is successful and subsequent requests act as that identity

#### Scenario: Switching to an identity that doesn't belong to the session
- **WHEN** a client requests a switch to an identity not associated with its established session
- **THEN** the response is a client error, and the active identity is unchanged

### Requirement: Sign out
The system SHALL allow a client to end its session, after which requests that require being signed
in are no longer served as that account.

#### Scenario: Signing out
- **WHEN** a client with an established session requests sign-out
- **THEN** the response is successful, and a subsequent request that requires being signed in is
  treated as signed out, not as the previous account

### Requirement: Signed-in-only requests without a session are rejected
The system SHALL reject a request to any signed-in-only capability (library, playlists-write,
ratings, settings, lyrics-source-preference, Last.fm, blocked artists) when the caller has no
established session, with a clear client error rather than a server crash or a response as if
signed out silently succeeded.

#### Scenario: A signed-in-only request with no session
- **WHEN** a client with no established session requests a signed-in-only action (e.g. viewing
  their library)
- **THEN** the response is a client error identifying that sign-in is required
