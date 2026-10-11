## Purpose

Lets a browser client fetch lyrics for a track, choose an alternate source, adjust sync offset,
and list available providers, over plain HTTP.

## ADDED Requirements

### Requirement: Fetch lyrics for a track
The system SHALL respond with lyrics for a given track, when available from any configured
provider, and SHALL respond with an explicit "no lyrics" result rather than an error when no
provider has a match.

#### Scenario: A track with lyrics available
- **WHEN** a client requests lyrics for a track at least one provider has
- **THEN** the response is successful and includes the lyrics

#### Scenario: A track with no lyrics anywhere
- **WHEN** a client requests lyrics for a track no provider has
- **THEN** the response is successful and indicates no lyrics are available — not an error

### Requirement: Choose an alternate lyrics source
The system SHALL let a client request lyrics for a track from a specific provider instead of the
default selection order.

#### Scenario: Choosing a specific provider
- **WHEN** a client requests a track's lyrics from a named provider that has a match
- **THEN** the response is successful and reflects that provider's lyrics, not the default choice

### Requirement: Adjust lyrics sync offset
The system SHALL let a client store a per-track timing offset applied to synced lyrics.

#### Scenario: Setting an offset
- **WHEN** a client sets an offset for a track
- **THEN** a subsequent lyrics fetch for that track reflects the stored offset

### Requirement: List lyrics providers
The system SHALL respond with the list of configured lyrics providers.

#### Scenario: Listing providers
- **WHEN** a client requests the provider list
- **THEN** the response is a successful list
