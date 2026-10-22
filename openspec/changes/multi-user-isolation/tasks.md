## 1. Database: account-scoped schema

- [ ] 1.1 Add the `settings_v2`-style rebuild migration (composite `(account_id, key)` primary key, backfilled from the current single `active_account`) per design.md's Migration Plan, and verify a fresh DB creates the new shape directly while an existing single-account DB's settings survive the migration with their values intact (unit test with a pre-migration fixture DB).
- [ ] 1.2 Add an `account_id` column to `plays` (`ALTER TABLE ... ADD COLUMN`, ignoring the expected "duplicate column" error on a fresh DB, matching this file's existing migration pattern) and backfill existing rows the same way as 1.1, and verify via a migration unit test.
- [ ] 1.3 Update every `Db` method that reads/writes `settings` or `plays` to take an `account_id`/`account_key` parameter, and verify existing callers fail to compile until updated (forces every call site to be touched deliberately, not missed).
- [ ] 1.4 Verify `playlist_track` and `local_tracks`/`local_playlists`/`local_playlist_tracks` are deliberately left unscoped (no schema change) and add a short code comment at each explaining why, per design.md's Non-Goals.

## 2. Shared vs. per-account state split

- [ ] 2.1 Introduce `AccountSession { it: InnerTube, account_key: String }` and a `Shared` struct holding the process-wide `Clients`/`cipher`/`potoken`/`Orchestrator`/`Db`, and verify `cargo check -p limusic-core` passes with the new types defined but not yet wired in.
- [ ] 2.2 Change `Orchestrator::resolve`/`resolve_video` (and any other method currently reading `self.it`) to take `&InnerTube` as a parameter instead of holding one at construction, and verify the existing orchestrator unit tests pass unchanged (they already construct their own `InnerTube`/`Clients` per test, so this should be close to mechanical).
- [ ] 2.3 Update `AppState::new`'s signature and every construction call site (`crates/server/src/main.rs`) to build the new `Shared`-shaped state without a bound-in `InnerTube`, and verify `cargo build -p server` succeeds.

## 3. Session store and per-account lookup

- [ ] 3.1 Add `accounts: Arc<RwLock<HashMap<String, Arc<AccountSession>>>>` to `ServerState`, with a method to get-or-create an `AccountSession` for a given `account_key`, and verify a unit test exercises concurrent get-or-create calls for the same key returning the same `Arc`.
- [ ] 3.2 Update `RequireSession::from_request_parts` to resolve session id → `account_key` (unchanged) → look up the matching `AccountSession` from the new map (removing the old single `state.app.it.is_logged_in()` global check), rejecting as unauthenticated when the account_key has no live session, and verify with a unit test covering: valid session with a live account succeeds; valid session whose account was never established (process restart) is rejected.
- [ ] 3.3 Thread the resolved `AccountSession` through to every route handler that currently reads `state.app.it` directly, and verify `cargo build -p server` succeeds with no remaining direct reads of a single global `InnerTube`.

## 4. Sign-in / sign-out isolation

- [ ] 4.1 Update `POST /api/auth/cookie` to establish or reuse an `AccountSession` keyed by the signed-in account's `account_key` without touching any other account's existing `AccountSession`, and verify with an integration test: sign in as account A, then account B, then confirm a request under A's session cookie still acts as A.
- [ ] 4.2 Update `POST /api/auth/signout` to remove only the calling session (and, once no other session references that account, its `AccountSession`) rather than clearing the whole `SessionStore`, and verify with an integration test: two concurrent sessions (A, B), sign out A, confirm B's session still works and A's no longer does.
- [ ] 4.3 Update the new `http-auth` spec's scenarios (both already written in this change's delta spec) to pass as integration tests.

## 5. Watch-history ping account-scoping

- [ ] 5.1 Add the resolving `account_key` to `audio_proxy::Registration` alongside its existing `ping`/`cpn`, set at `register()` time, per design.md Decision 4.
- [ ] 5.2 Change `take_ping` to accept the caller's `account_key` and only return a ping when it matches the registration's, and verify with a unit test: account A resolves and registers a ping, a `take_ping` call with account B's key returns `None`, the same call with account A's key returns the ping.
- [ ] 5.3 Update `POST /api/playback/register`'s handler to pass the caller's `account_key` (from its `RequireSession`) into `take_ping`, and to fire `register_playback` through that same account's `AccountSession.it` rather than a shared one, and verify with an integration test covering two accounts both playing the same video concurrently.

## 6. Capacity documentation

- [ ] 6.1 Add the concrete concurrent-user capacity target (and what resource bounds it, per design.md) to `README.md`'s self-hosting section, and verify the `multi-user-isolation` spec's "Looking up the capacity target" scenario is satisfied by pointing to that exact location.

## 7. Verification

- [ ] 7.1 Add an integration test exercising the `multi-user-isolation` spec's core scenario end-to-end: two accounts signed in concurrently, each creates a playlist and changes a setting, and each one's library/playlist/settings listing contains only its own data.
- [ ] 7.2 Run the full existing `cargo test --all` suite and confirm no regression in single-account use (today's one-account deployments must keep working exactly as before).
- [ ] 7.3 Manually verify against the real deployed VPS instance: sign in as one real account, confirm existing functionality (search, play, library) is unaffected before and after this change ships (no second real account available for this step, so this is a regression check, not a concurrency test — the automated test in 7.1 covers concurrency).
