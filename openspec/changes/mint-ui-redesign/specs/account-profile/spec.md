## Purpose

Defines a mobile-reachable Profile page: viewing the current account, switching between saved
accounts, signing out, and reaching the flow to add another account — behavior that exists today
only inside the desktop sidebar's account dropdown.

## ADDED Requirements

### Requirement: The current account's identity is visible on the Profile page
The Profile page SHALL display the signed-in account's name, email/handle, and avatar.

#### Scenario: Viewing Profile while signed in
- **WHEN** a signed-in user opens the Profile page
- **THEN** their name, email or handle, and avatar are displayed

### Requirement: Every saved account is listed and switchable
The Profile page SHALL list every saved account (per the existing saved-accounts feature) besides
the active one, and selecting one SHALL switch the active session to it without requiring a fresh
sign-in.

#### Scenario: Switching to a different saved account
- **WHEN** the user selects a different saved account from the Profile page's list
- **THEN** that account becomes the active session, with no cookie re-entry required

### Requirement: Adding another account is reachable from Profile
The Profile page SHALL provide an entry point into the cookie sign-in flow for adding a new
account, distinct from the mandatory sign-in gate (this entry point assumes an existing session is
already active).

#### Scenario: Adding a second account while already signed in
- **WHEN** a signed-in user selects "Add another account" from Profile
- **THEN** they reach the cookie-paste sign-in flow, and completing it adds a new saved account
  without signing them out of the one they started from

### Requirement: Signing out is reachable from Profile
The Profile page SHALL provide a sign-out action for the active account.

#### Scenario: Signing out from Profile
- **WHEN** the user selects sign out from the Profile page
- **THEN** the active session ends and the user is returned to the sign-in gate (`web-signin-gate`)
  if no other session remains, or to the next saved account if one does
