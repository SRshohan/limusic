## Purpose

Lets a signed-in browser client read and write their server-side settings, list available stream
clients, and clear server-side caches, over plain HTTP. Settings with no browser equivalent (app
icon path, allowed font file) are explicitly excluded — see design.md.

## ADDED Requirements

### Requirement: Read and write settings
The system SHALL let a signed-in client read the current value of a setting by key, and write a
new value for a key, persisted against their account.

#### Scenario: Writing then reading a setting
- **WHEN** a signed-in client writes a value for a settings key, then requests that key
- **THEN** the value returned matches what was written

#### Scenario: Reading a setting that was never set
- **WHEN** a signed-in client requests a settings key with no stored value
- **THEN** the response is successful, indicating no value is set — not an error

### Requirement: List available stream clients
The system SHALL respond with the set of InnerTube client identities currently usable for
streaming-related requests.

#### Scenario: Listing clients
- **WHEN** a signed-in client requests the list of stream clients
- **THEN** the response is a successful list

### Requirement: Clear server-side caches
The system SHALL let a signed-in client clear their server-side caches (e.g. cached stream URLs,
cached lyrics).

#### Scenario: Clearing caches
- **WHEN** a signed-in client requests cache clearing
- **THEN** the response is successful and subsequent requests that would have used a cached value
  fetch fresh instead
