## ADDED Requirements

### Requirement: Search across all categories over HTTP
The system SHALL accept an HTTP request carrying a text query and respond with results grouped by
category (e.g. songs, videos, albums, artists, playlists), without requiring the caller to be
signed in.

#### Scenario: A query with results across categories
- **WHEN** a client sends an all-categories search request with a query that matches real content
- **THEN** the response is a successful set of results grouped by category

#### Scenario: A query with no results
- **WHEN** a client sends an all-categories search request with a query that matches nothing
- **THEN** the response is a successful, empty result set — not an error

#### Scenario: A missing or empty query
- **WHEN** a client sends an all-categories search request with no query or an empty one
- **THEN** the response is a client error, not a server crash or an empty-success result treated
  as a valid search

### Requirement: Get search suggestions over HTTP
The system SHALL accept an HTTP request carrying a partial text query and respond with typeahead
suggestions, without requiring the caller to be signed in and without recording the query against
any search history (only a query the user explicitly submits is ever recorded, never a typeahead
keystroke).

#### Scenario: A partial query with suggestions
- **WHEN** a client requests suggestions for a partial query that YouTube Music has completions for
- **THEN** the response is a successful list of suggestions

#### Scenario: An empty query
- **WHEN** a client requests suggestions for an empty query
- **THEN** the response is a successful, empty list of suggestions — not an error (there is simply
  nothing to suggest yet, unlike a submitted search)

### Requirement: Search within one category over HTTP
The system SHALL accept an HTTP request carrying a text query and a category (albums, artists, or
playlists) and respond with that category's matching results, for the search page's "show more"
view of a single category.

#### Scenario: A valid query and category
- **WHEN** a client requests one category's results for a query that matches real content in that
  category
- **THEN** the response is a successful list of results for that category

#### Scenario: An unsupported category value
- **WHEN** a client requests a category value that is not one of the supported categories
- **THEN** the response is a client error, not a server crash

### Requirement: Search videos over HTTP
The system SHALL accept an HTTP request carrying a text query and respond with a list of matching
videos, without requiring the caller to be signed in.

#### Scenario: A query with video results
- **WHEN** a client sends a video search request with a query that matches real videos
- **THEN** the response is a successful list of videos

#### Scenario: A query with no video results
- **WHEN** a client sends a video search request with a query that matches no videos
- **THEN** the response is a successful, empty list — not an error
