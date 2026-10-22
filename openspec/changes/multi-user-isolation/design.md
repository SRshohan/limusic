## Context

`crates/server`'s `ServerState { app: Arc<AppState>, sessions: Arc<SessionStore> }` and
`crates/core`'s `AppState` (`state.rs`) were built single-account-first: `AppState::new` takes one
`InnerTube` and wraps it with one `Db`, one `Orchestrator`, one `Broadcaster`. `SessionStore` maps
a session id to an `account_key`, but `RequireSession` additionally gates every request on
`state.app.it.is_logged_in()` — a single, process-wide flag. Signing in replaces that one
`InnerTube`'s cookie jar outright, so a second sign-in silently evicts the first.

The actual SQLite schema (`crates/core/src/db.rs`) confirms the same assumption runs through every
table that matters here:
- `settings` is a bare `(key, value)` table. Every key read/written today — `active_account`,
  `session_cookie`, `data_sync_id`, `visitor_data`, `account_json`, `selected_identity_json`,
  `quality`, `disabled_stream_clients`, `lastfm_session_key`, `lastfm_username`,
  `account_selection_pending` — is really per-person data living in a table with no notion of
  "per person."
- `plays` (local play counts, feeds "On Repeat") has no account column either: every account's
  plays land in the same feed today.
- `accounts` is already keyed by `account_key` and already stores more than one account's
  credentials side by side — this table is most of the way there already. What's missing is simply
  that only one of them is ever "active."
- `local_tracks` / `local_playlists` / `local_playlist_tracks` are **deliberately** unscoped
  (issue #251: "no account needed" — a playlist of files on this machine isn't YouTube data). That
  rationale extends cleanly to multi-user on one server: these stay shared infrastructure, not
  per-account data. Treated as a Non-Goal below, not an oversight.
- `stream_url_cache` (with `ping_url`/`ping_client` columns, suggesting an earlier watch-history
  design) is defined but not read or written anywhere outside `db.rs` itself — dead code today. Not
  touched by this change; if it's ever revived it inherits the same treatment as `audio_proxy`
  below.

Also relevant: this session's own earlier work added `crates/core/src/media/audio_proxy.rs`'s
`take_ping`/`register()` for watch-history pings, keyed by `video_id` only, with the actual
`register_playback` call made through `state.app.it` — today's one, global account. That call site
needs to change as part of this work (see Decisions).

## Goals / Non-Goals

**Goals:**
- More than one YouTube account signed in and acting independently, at the same time, on one
  running `crates/server` process.
- Account-scoped `settings`, `plays`, and playlist-membership data, migrated from today's single
  implicit "active account" to explicit per-account rows.
- A concrete, stated concurrent-user capacity target and the resource it's bounded by.

**Non-Goals:**
- Multiple *simultaneous* identities for the *same* physical user acting at once — that's the
  existing "saved accounts, switch between them" feature (`get_google_accounts`/`switch_account`),
  unchanged. This change is about different people, not one person's several logins.
- Scoping `local_tracks`/`local_playlists`/`local_playlist_tracks` per account — see Context;
  staying shared is the deliberate choice, not a gap.
- A hard, enforced admission limit that rejects the Nth concurrent sign-in. The capacity target
  below is a documented design target, not a new rejection behavior to build.
- Any change to `src-tauri` (desktop keeps its own single-account-per-process model) or to
  `crates/sync-server`/Listen Together.

## Decisions

### 1. Split `AppState` into shared infrastructure + per-account session state

Today's single struct splits into:
- **Shared, process-wide** (unchanged in spirit, just no longer bundled with account state):
  `Clients` (static client definitions), `Arc<CipherDeobfuscator>`, `Arc<PoTokenGenerator>`, the
  `Orchestrator`, and the single `Db` handle. None of these are tied to a login.
- **Per-account** (`AccountSession`, new): `it: InnerTube` (that account's cookie jar + healing
  state) and the account's identifier (`account_key`). Created lazily on first sign-in for a given
  `account_key`, kept alive for as long as any session references it.

`Orchestrator::resolve` already only needs `self.it` for `is_logged_in()`/`visitor_data()`/
`player(...)` calls — none of that requires the orchestrator to *own* a specific account's
`InnerTube` at construction time. It takes `&InnerTube` as a per-call parameter instead, so one
`Orchestrator` (and the one `cipher`/`potoken` runtime behind it) serves every account. This is
the smallest change that gets concurrency right: the heaviest shared resources stay singular, only
the account-identity-bearing parts become per-call.

`ServerState` becomes `{ shared: Arc<Shared>, sessions: Arc<SessionStore>, accounts:
Arc<RwLock<HashMap<AccountKey, Arc<AccountSession>>>> }`. `RequireSession` resolves session id →
`account_key` (unchanged) → looks up the `Arc<AccountSession>` from the map, rejecting as
unauthenticated if the account_key has no live entry (a process restart loses in-memory
`AccountSession`s exactly like it already loses the single session today — not a new regression,
see the already-known "session doesn't survive restart" gap).

**Alternative considered**: one whole `AppState`-shaped struct per account (the mirror-image of
today's design, just N of them). Rejected — it would duplicate the `Orchestrator`/`cipher`/
`potoken` stack N times, multiplying the heaviest, V8-backed part of the process for isolation
that only actually needs to apply to the `InnerTube` cookie jar and the DB rows.

### 2. One SQLite file, account-scoped rows — not one file per account

`settings` and `plays` gain an `account_id TEXT NOT NULL` column (`ALTER TABLE ... ADD COLUMN`,
same pattern this file already uses for `loudness_db`/AlbumArtist — errors on a fresh DB are
expected and ignored). `settings`' primary key changes from `(key)` to `(account_id, key)`, which
SQLite can't do with `ALTER TABLE` alone: rebuild into a new table with the composite key, copy
existing rows forward (see Migration Plan), rename. `playlist_track` (YouTube-side playlist
membership cache, keyed by YouTube's own globally-unique `playlist_id`) needs no new column — two
users referencing the same real YouTube playlist should see the same cached membership; there's no
collision to prevent.

**Alternative considered**: one SQLite file per account (`limusic-{account_id}.sqlite`). Rejected
for this scale — it means N connection pools, N backup targets, and doesn't actually simplify the
migration (the composite-key rebuild above is a one-time cost either way), for an operational
complexity that only pays off at a scale this design isn't targeting.

### 3. `audio_proxy`/`video_proxy` stay global and video_id-keyed — an accepted, bounded risk

A resolved stream URL for video X is the same bytes regardless of who asked for it, so keeping
these caches shared (not duplicated per account) avoids redundant resolve work for concurrent users
watching the same thing. Accepted risk: if account A's resolve of an age-restricted or
members-only video succeeds and account B then requests the same `video_id` while that entry is
still live, B's `/media/stream/{id}` request is served from A's cached entry rather than failing or
re-resolving under B's own (possibly lesser) access. For this design's target (a trusted household/
friend group, not a public multi-tenant service), that's judged acceptable: it costs nothing a
user's own account wouldn't also expose to anyone they shared a device with, and the alternative
(duplicating the registry per account) undoes the point of a shared cache for the common case
(everyone resolving the same popular tracks).

### 4. Watch-history pings become account-aware, not just session-aware

`audio_proxy::take_ping`'s claim must be account-scoped: a ping registered by account A's resolve
can only be fired by a request authenticated as account A, not whichever account's request happens
to reach `/api/playback/register` first. Concretely, `Registration` gains the resolving
`account_key` alongside its existing `ping`/`cpn`; `take_ping(video_id, caller_account_key)` only
returns a ping when the caller matches, and the route handler's `register_playback` call uses that
specific account's `AccountSession.it`, not a single shared one. This follows directly from
Decision 3: once the stream cache is shared, the ping it carries must not be.

## Risks / Trade-offs

- [A long-lived `AccountSession` per concurrently-active account multiplies `InnerTube`'s own
  memory footprint (cookie jar, in-flight request state) by the number of concurrent accounts] →
  Bounded by the stated capacity target (tens, not thousands); `InnerTube` itself is far lighter
  than the shared cipher/potoken V8 runtimes this design deliberately keeps singular.
- [Cached stream URLs crossing account boundaries, per Decision 3] → Accepted for this design's
  scale; documented explicitly rather than silently shipped.
- [The composite-primary-key rebuild of `settings` is a real schema migration on an existing
  deployment's live data, not just an `ADD COLUMN`] → Mitigated by the Migration Plan below:
  back up first, rebuild offline during a short maintenance window, backfill existing rows under
  the one account that was "active" at migration time (today's only account, in every pre-existing
  single-user deployment).
- [`RwLock<HashMap<AccountKey, Arc<AccountSession>>>` read-lock contention under many concurrent
  requests] → Read-mostly access pattern (a write only happens on a brand-new account's first
  sign-in); at the stated capacity target this isn't expected to be a measurable bottleneck, and is
  cheap to revisit (e.g. a sharded map) if it ever is.

## Migration Plan

1. Ship the schema change behind the existing `open_or_quarantine` startup path: on open, check
   whether `settings` already has the composite `(account_id, key)` primary key; if not, create
   `settings_v2` with the new shape, copy every existing row into it tagged with whatever
   `active_account` currently resolves to (every pre-existing deployment has exactly one account,
   by definition of today's single-account design — this backfill is exact, not a guess), drop the
   old table, rename. Same approach for `plays`.
   Take this exact approach instead of an in-place migration because SQLite cannot alter a
   `PRIMARY KEY` in place, and because resuming output that is already correct under a fresh
   rebuild test (CI) matters as much as mutating a VPS's one disk file does.
2. Deploy the new binary. A single already-running account keeps working unmodified (it's both the
   source and sole destination of the backfill) — this is a feature-adding deploy, not a breaking
   one for any existing single-user instance.
3. Rollback: the quarantine mechanism this file already uses for a corrupt DB (`open_or_quarantine`)
   covers a failed migration the same way — the previous binary + the pre-migration file (kept as
   a `.bak` by the migration step before it touches anything) is the rollback path.

## Open Questions

- Whether `account_selection_pending` (today a single global settings flag for the "choose which
  channel to sign in as" flow) needs its own account-scoping treatment, or whether it's
  inherently a pre-sign-in, not-yet-account-scoped concern that's fine to stay global — doesn't
  change the schema decision above either way, so safe to resolve during implementation rather
  than here.
