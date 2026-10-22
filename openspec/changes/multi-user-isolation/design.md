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
- **Narrowed during implementation**: `switch_google_account` (switching to a *different saved
  account entirely*, as opposed to `switch_account`'s within-one-account channel switching) mutates
  its live session's cookie jar in place today — safe with one account per process, not safe once
  a different session might reference the same account's state concurrently. Making it safe means
  "look up or create a different `AccountSession`, then repoint only the calling session at it," a
  real redesign, not a scoping change. Out of this change's scope: it returns a clear
  not-yet-supported error instead, tracked as explicit follow-up work.
- Scoping `local_tracks`/`local_playlists`/`local_playlist_tracks` per account — see Context;
  staying shared is the deliberate choice, not a gap.
- A hard, enforced admission limit that rejects the Nth concurrent sign-in. The capacity target
  below is a documented design target, not a new rejection behavior to build.
- Any change to `src-tauri` (desktop keeps its own single-account-per-process model) or to
  `crates/sync-server`/Listen Together.

## Decisions

### 1. Split `AppState` into shared infrastructure + per-account session state

**Revised after implementation research** (the original version of this decision underestimated
the surface area — corrected here rather than left inaccurate): `crates/core/src/state.rs`'s
auth-flow methods are not a few isolated reads of `self.it` — `sign_in`/`sign_in_inner`,
`account_identities`, `switch_account`/`switch_account_inner`, `activate_identity`,
`persist_selected_identity`, `restore_auth_transport`, `persist_visitor_data`,
`switch_google_account`/`switch_google_account_inner`, `forget_account`, `clear_session`,
`sync_active_account`, `persist_rotated_cookie`, `keep_session_alive`, and `account_snapshot` —
essentially the whole auth module, around a dozen methods — all read and mutate `self.it` and the
account-identity-bearing parts of `self.db` together, as one implicit account's state. This means
`AccountSession` isn't a thin `{ it, account_key }` pair with those methods left on `AppState`
unchanged; those methods themselves move onto `AccountSession`, parameterized by its own
`account_key` where they touch `self.db`.

Note also: `crates/core` was already fork-and-frozen from `src-tauri` (per `signed-in-http-parity`)
— `crates/core/src/state.rs` and `src-tauri/src/state.rs` are independent copies, not a shared
module. Restructuring the former cannot affect the latter; desktop's single-account model is
structurally unreachable from this change, not just unreferenced by its tasks.

After the split:
- **Shared, process-wide**: `Clients` (static client definitions), `Arc<CipherDeobfuscator>`,
  `Arc<PoTokenGenerator>`, the `Orchestrator`, and the single `Db` handle. None of these are tied
  to a login.
- **Per-account** (`AccountSession`, new): `it: InnerTube`, `account_key: String`, its own
  `auth: tokio::sync::Mutex<()>` (today's single process-wide "one auth transition at a time" lock
  moves here too — see Risks), and the former `AppState` auth methods listed above, now methods on
  `AccountSession` instead. Created lazily on first sign-in for a given `account_key`, kept alive
  for as long as any session references it.

`Orchestrator::resolve`/`resolve_video`/`headers_for` already only need `self.it` for
`is_logged_in()`/`visitor_data()`/`cookie()`/`player(...)` calls — none of that requires the
orchestrator to *own* a specific account's `InnerTube` at construction time. They take `&InnerTube`
as a per-call parameter instead, so one `Orchestrator` (and the one `cipher`/`potoken` runtime
behind it) serves every account.

`ServerState` becomes `{ shared: Arc<Shared>, sessions: Arc<SessionStore>, accounts:
Arc<RwLock<HashMap<AccountKey, Arc<AccountSession>>>> }`. `RequireSession` resolves session id →
`account_key` (unchanged) → looks up the `Arc<AccountSession>` from the map, rejecting as
unauthenticated if the account_key has no live entry (a process restart loses in-memory
`AccountSession`s exactly like it already loses the single session today — not a new regression,
see the already-known "session doesn't survive restart" gap), and hands the resolved
`Arc<AccountSession>` to the handler (not just the bare `account_key` string) so handlers don't
each re-look it up.

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

**Per-key classification** (resolved now — implementation research found every key in active use,
settling the ambiguity design-time rather than leaving it to be guessed mid-implementation):
- **Per-account** (gets the new `account_id` column): `session_cookie`, `visitor_data`,
  `data_sync_id`, `selected_identity_json`, `account_json` (today's "active account" projection,
  per account now), `quality`, `disabled_stream_clients`, `lastfm_session_key`, `lastfm_username`,
  `locale`, `hide_videos` (these last two are structurally forced per-account regardless: both also
  call `self.it.set_locale(...)`/`set_hide_videos(...)` directly, and `it` is becoming per-account),
  `playlist_index_synced_at`, `account_selection_pending`.
- **Stays global** (no account scoping, same single row for the whole process): `potoken_session`
  (BotGuard session-token persistence — infrastructure state, the same category as `cipher`/
  `potoken` themselves, not account data), `blocked_artists` (see Decision 5), and **corrected
  during implementation** `lyrics_providers` — not just a personal preference as first classified:
  `GET /api/lyrics` reads it directly and is itself an anonymous, ungated route (fetching lyrics
  never required sign-in), so like `blocked_artists` there's no session to scope it by. Its other
  reader, the signed-in `/api/settings` allowlist, reads the same global row either way.
- **Removed, not migrated**: `active_account`. Under one account per process this setting named
  which `StoredAccount` row was "the" live one; under concurrent accounts there's no single active
  one to name — a session's `account_key` (from `SessionStore`) already identifies which account a
  request is for, directly, with no indirection through a settings row needed. Only
  `crates/core`'s copy loses this — `src-tauri`'s independent copy (see Decision 1's note on the
  fork) is untouched and keeps using it exactly as today.
- `blocked_artists` staying global is deliberate, not an inconsistency: `crates/server/src/
  blocked.rs`'s three routes currently take no `RequireSession` at all (a prior decision — a code
  comment references reconciling this with `http-blocked-artists`'s own spec), and this change
  doesn't add gating that capability never asked for. With no session to scope by, it stays what it
  already structurally is: one global, ungated setting — a server-wide content filter, not a
  personal one.

**Alternative considered**: one SQLite file per account (`limusic-{account_id}.sqlite`). Rejected
for this scale — it means N connection pools, N backup targets, and doesn't actually simplify the
migration (the composite-key rebuild above is a one-time cost either way), for an operational
complexity that only pays off at a scale this design isn't targeting.

### 3. `audio_proxy`/`video_proxy` keyed by `(account_key, video_id)` — not shared, and not an
accepted risk

**Revised after implementation research**: the original version of this decision kept these
registries keyed by `video_id` alone and accepted the consequence as a content-visibility risk
(account B's request being served account A's cached resolve for the same video). Research
surfaced a stronger problem than that: a *video_id*-only key means a **second concurrent `register()`
call for the same video overwrites the first's entry outright** — including its `headers` (the
cookie an upload stream needs to authenticate as the account that resolved it). Two different
accounts resolving the same popular track around the same moment isn't a rare edge case on a
shared server; it's an expected one. The consequence isn't "the wrong account's cached data might
get read" (a visibility risk), it's "an in-flight stream can start being served under a *different*
account's credentials mid-request" — a correctness bug, not a documented trade-off.

Fixed by keying both registries by `(account_key, video_id)` instead of `video_id` alone. Each
account gets its own cache entry per video; no overwrite, no cross-account credential leak. The
cost is the efficiency `video_id`-only keying was for — two accounts resolving the same track no
longer share one cached entry — judged acceptable at this design's scale (resolving is not the
expensive, rate-limited part of the pipeline the way cipher/BotGuard are, which stay fully shared
regardless).

### 4. Watch-history pings are account-aware as a consequence of Decision 3, not a separate fix

With the registries keyed by `(account_key, video_id)`, `audio_proxy::take_ping` naturally takes the
caller's `account_key` as part of its lookup key — there is no longer a cross-account ping to guard
against separately, because there is no longer a cross-account registration to find in the first
place. The route handler's `register_playback` call uses that specific account's
`AccountSession.it`.

## Risks / Trade-offs

- [A long-lived `AccountSession` per concurrently-active account multiplies `InnerTube`'s own
  memory footprint (cookie jar, in-flight request state) by the number of concurrent accounts] →
  Bounded by the stated capacity target (tens, not thousands); `InnerTube` itself is far lighter
  than the shared cipher/potoken V8 runtimes this design deliberately keeps singular.
- [`(account_key, video_id)` keying (Decision 3) means two accounts resolving the same popular
  track no longer share one cached entry, redoing resolve work that `video_id`-only keying would
  have avoided] → Accepted: resolving isn't the expensive, rate-limited part of this pipeline
  (cipher/BotGuard stay fully shared regardless); a correctness bug was not an acceptable price for
  that efficiency.
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

### 5. Account-switching keeps its behavior; "active" moves from a global setting to the session

**Found during implementation**: the existing "switch between several of your own saved Google
logins" feature (`restore_account`, `switch_account_inner`, `switch_google_account_inner`) is built
on the global `active_account` setting Decision 2 removes — not a coincidental user of it. Its
Non-Goal status (unchanged for one user acting alone) doesn't mean untouched code; it means the
same observable behavior. Resolution: `SessionStore`'s session → `account_key` mapping already *is*
"which account is active," just per-session instead of global. These methods' "make this the
active account" side effect becomes "update the calling session's own entry in `SessionStore`"
instead of a global settings write — which means they need the calling session's id threaded in
as a parameter, not just account state.

## Open Questions

None remaining — `account_selection_pending`'s scoping is resolved in Decision 2's per-key
classification (per-account: the pending multi-channel selection belongs to whichever account's
sign-in triggered it).
