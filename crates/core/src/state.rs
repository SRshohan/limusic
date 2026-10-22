//! Account/session state, trimmed from src-tauri's `AppState` (openspec/changes/
//! signed-in-http-parity). context/11.
//!
//! openspec/changes/multi-user-isolation split this into two: `Shared` (process-wide — cipher,
//! PoToken, the orchestrator, the `Clients` table, the one `Db` handle — none of it tied to a
//! login) and `AccountSession` (one per concurrently signed-in account: its own `InnerTube` cookie
//! jar, its own auth-transition lock, and every method that used to assume there was only ever one
//! of either).

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use std::sync::Arc;

use innertube::{AccountIdentity, AccountInfo, AudioQuality, Clients, InnerTube};

use crate::db::{now_secs, Db, StoredAccount, GLOBAL_ACCOUNT};

/// Synthetic browseId for the On Repeat playlist. Not a YouTube id: `get_playlist` intercepts it
/// and builds the page from local play counts, so it must never collide with a real browseId
/// (`VL…` / `MPRE…` / `RD…`).
pub const ON_REPEAT_ID: &str = "LIMUSIC_ON_REPEAT";
/// How far back On Repeat looks. A month is long enough to survive a quiet week and short enough
/// that the list still turns over with what you're actually playing.
pub const ON_REPEAT_WINDOW_SECS: i64 = 30 * 24 * 60 * 60;
/// How many songs it holds.
pub const ON_REPEAT_LIMIT: usize = 20;

/// A playlist kept on this machine (issue #251) is the browseId `LOCALPLAYLIST:<row id>`. Like On
/// Repeat, the playlist commands intercept it and answer from SQLite, and YouTube never sees one.
pub const LOCAL_PLAYLIST_PREFIX: &str = "LOCALPLAYLIST:";

pub fn is_local_playlist(id: &str) -> bool {
    id.starts_with(LOCAL_PLAYLIST_PREFIX)
}

/// Process-wide state, shared by every concurrently signed-in account (openspec/changes/
/// multi-user-isolation). None of this is tied to a login: the cipher/PoToken runtimes decode
/// YouTube's public player.js and mint anonymous bot-attestation tokens respectively, the
/// orchestrator takes an `&InnerTube` per call instead of owning one, and the one `Db` handle's
/// account-scoped tables are scoped by the caller's `account_key`, not by which `Shared` they came
/// through (there is only ever one `Shared`).
pub struct Shared {
    pub clients: Clients,
    pub db: Arc<Db>,
    /// Replaces `AppHandle::emit` — see `crate::events`.
    pub events: crate::events::Broadcaster,
    /// Last.fm scrobbler. Same feed again; parks until a session key is set (titlebar button).
    pub lastfm: crate::lastfm::LastfmHandle,
    /// Stream resolution (cipher/PoToken/client-fallback pipeline).
    pub orchestrator: Arc<crate::media::orchestrator::Orchestrator>,
    /// The one `InnerTube` every anonymous route (browse, search, an unauthenticated media
    /// resolve) uses — never signed in, shared the same way `cipher`/`potoken` are, for the same
    /// reason: nothing about an anonymous lookup is account-specific.
    pub anonymous_it: InnerTube,
}

impl Shared {
    /// `app_data_dir` is where the cipher caches `player.js` and its config registry snapshot
    /// (`crates/core/src/cipher/{fetcher,config}.rs`) — the server's equivalent of the desktop
    /// app's app-data directory, not reused for anything else here.
    pub fn new(
        clients: Clients,
        db: Arc<Db>,
        lastfm: crate::lastfm::LastfmHandle,
        app_data_dir: &std::path::Path,
    ) -> Self {
        let config = Arc::new(crate::cipher::PlayerConfigStore::new(app_data_dir));
        let cipher = Arc::new(crate::cipher::CipherDeobfuscator::new(app_data_dir, config));
        let potoken = Arc::new(crate::potoken::PoTokenGenerator::new(db.clone()));
        let orchestrator =
            Arc::new(crate::media::orchestrator::Orchestrator::new(clients.clone(), cipher, potoken));
        let anonymous_it = InnerTube::new(innertube::Session::default(), None)
            .expect("failed to build the anonymous InnerTube client");
        Shared {
            clients,
            db,
            events: crate::events::Broadcaster::default(),
            lastfm,
            orchestrator,
            anonymous_it,
        }
    }

    /// The stored `quality` setting for `account_id` (`GLOBAL_ACCOUNT` for an anonymous caller),
    /// same mapping as the desktop app's own `AppState::quality`.
    pub fn quality(&self, account_id: &str) -> AudioQuality {
        match self.db.get_setting(account_id, "quality").as_deref() {
            Some("LOW") => AudioQuality::Low,
            Some("AUTO") => AudioQuality::Auto,
            _ => AudioQuality::High,
        }
    }

    /// User-disabled stream clients for `account_id` — same comma-separated
    /// `disabled_stream_clients` setting and `LIMUSIC_DISABLED_CLIENTS` env override as the
    /// desktop app's own `AppState::disabled_clients`.
    pub fn disabled_clients(&self, account_id: &str) -> HashSet<String> {
        let raw = std::env::var("LIMUSIC_DISABLED_CLIENTS")
            .ok()
            .or_else(|| self.db.get_setting(account_id, "disabled_stream_clients"))
            .unwrap_or_default();
        raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    }
}

/// One concurrently signed-in account (openspec/changes/multi-user-isolation): its own YouTube
/// session and its own "one auth transition at a time" lock, looked up by `account_key` from the
/// calling session's cookie (`crates/server/src/session.rs`). Every `self.db.*` call below passes
/// `self.account_key` as the scope, so two different accounts' settings/history never collide.
pub struct AccountSession {
    pub it: InnerTube,
    pub account_key: String,
    /// One authentication transition at a time *for this account*: [`Self::sign_in`] (healing),
    /// [`Self::switch_account`], [`Self::sign_out`]. Each reads the cookie and identity, awaits
    /// YouTube, then writes both to the transport and the database, so two of them overlapping
    /// lets the loser land last and win. Async because it is held across those awaits, which a
    /// std `Mutex` cannot be. Scoped to this one account now, not the whole process: a slow auth
    /// transition on one account no longer blocks every other concurrently signed-in account's own
    /// transitions.
    auth: tokio::sync::Mutex<()>,
}

/// Canonical persisted account selection. `data_sync_id` drives request delegation; every display
/// field beside it belongs to that same identity. `account_json` and the legacy `data_sync_id`
/// setting are written only as atomic projections of this model.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct SelectedIdentity {
    #[serde(default)]
    data_sync_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    handle: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    has_multiple_identities: bool,
}

pub enum SignInOutcome {
    Complete,
    SelectionRequired,
}

impl SelectedIdentity {
    fn from_account_info(info: AccountInfo, has_multiple_identities: bool) -> Option<Self> {
        Some(Self {
            data_sync_id: info.data_sync_id,
            name: Some(info.name?),
            handle: info.handle,
            email: info.email,
            thumbnail: info.thumbnail,
            channel_id: info.channel_id,
            has_multiple_identities,
        })
    }

    fn from_identity(
        identity: &AccountIdentity,
        refreshed: AccountInfo,
        has_multiple_identities: bool,
    ) -> Self {
        Self {
            data_sync_id: Some(identity.data_sync_id.clone()),
            name: refreshed.name.or_else(|| Some(identity.name.clone())),
            handle: refreshed.handle.or_else(|| identity.handle.clone()),
            email: refreshed.email.or_else(|| identity.email.clone()),
            thumbnail: refreshed.thumbnail.or_else(|| identity.thumbnail.clone()),
            channel_id: refreshed.channel_id.or_else(|| identity.channel_id.clone()),
            has_multiple_identities,
        }
    }

    fn account_json(&self, signed_in: bool) -> serde_json::Value {
        serde_json::json!({
            "signedIn": signed_in,
            "name": self.name,
            "handle": self.handle,
            "email": self.email,
            "thumbnail": self.thumbnail,
            "channelId": self.channel_id,
            "canSwitch": self.has_multiple_identities,
        })
    }

    fn as_account_identity(&self) -> Option<AccountIdentity> {
        Some(AccountIdentity {
            name: self.name.clone().unwrap_or_default(),
            handle: self.handle.clone(),
            email: self.email.clone(),
            thumbnail: self.thumbnail.clone(),
            channel_id: self.channel_id.clone(),
            data_sync_id: self.data_sync_id.clone()?,
            is_selected: false,
        })
    }
}

fn selected_identity_from_db(db: &Db, scope: &str) -> Option<SelectedIdentity> {
    db.get_setting(scope, "selected_identity_json")
        .and_then(|json| serde_json::from_str::<SelectedIdentity>(&json).ok())
        .filter(|identity| identity.name.is_some() || identity.data_sync_id.is_some())
        .or_else(|| {
            let account = db
                .get_setting(scope, "account_json")
                .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())?;
            let string = |key: &str| {
                account
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            Some(SelectedIdentity {
                data_sync_id: db.get_setting(scope, "data_sync_id").filter(|id| !id.is_empty()),
                name: string("name"),
                handle: string("handle"),
                email: string("email"),
                thumbnail: string("thumbnail"),
                channel_id: string("channelId"),
                has_multiple_identities: account
                    .get("canSwitch")
                    .and_then(serde_json::Value::as_bool)
                    // Pre-switcher account_json has no flag. Keep the action discoverable until
                    // the startup identity refresh determines whether this legacy user has one
                    // channel or several.
                    .unwrap_or(true),
            })
        })
}

fn auth_selection_pending(db: &Db, scope: &str) -> bool {
    db.get_setting(scope, "account_selection_pending").as_deref() == Some("true")
}

fn identity_selection_key(data_sync_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    "limusic-account-identity-v1".hash(&mut hasher);
    data_sync_id.hash(&mut hasher);
    format!("identity-{:016x}", hasher.finish())
}

fn identity_snapshot(identity: &AccountIdentity, selected: bool) -> serde_json::Value {
    serde_json::json!({
        "selectionKey": identity_selection_key(&identity.data_sync_id),
        "name": identity.name,
        "handle": identity.handle,
        "email": identity.email,
        "thumbnail": identity.thumbnail,
        "channelId": identity.channel_id,
        "selected": selected,
    })
}

impl AccountSession {
    /// A session with no live login yet — the starting point for a brand-new `account_key` the
    /// moment it's first seen (`ServerState`'s get-or-create), before anyone has signed in as it.
    pub fn anonymous(account_key: String) -> Self {
        AccountSession {
            it: InnerTube::new(innertube::Session::default(), None)
                .expect("failed to build InnerTube client"),
            account_key,
            auth: tokio::sync::Mutex::default(),
        }
    }

    // --- auth (context/15) ------------------------------------------------------------------

    /// Sign in with the Cookie header captured from the login webview (context/15 Path A), or
    /// (the web build's only login path) pasted directly. Builds a *fresh* `InnerTube` rather
    /// than mutating this account's existing one in place: the account_key isn't known until the
    /// cookie is read, so there's no existing session to safely mutate until this succeeds.
    /// `expect_account` is the SAPISID the caller read before it went off to fetch `cookie`. The
    /// session heal needs it: the user can switch accounts while it exports the jar, and signing
    /// in with what it found would drag them back to the account they just left.
    pub async fn sign_in(
        shared: &Shared,
        cookie: String,
        expect_account: Option<&str>,
    ) -> Result<(AccountSession, SignInOutcome), String> {
        innertube::without_healing(Self::sign_in_inner(shared, cookie, expect_account)).await
    }

    async fn sign_in_inner(
        shared: &Shared,
        cookie: String,
        expect_account: Option<&str>,
    ) -> Result<(AccountSession, SignInOutcome), String> {
        let cookie = cookie.trim().to_owned();
        let Some(sapisid) = innertube::cookie_sapisid(&cookie) else {
            return Err("Sign-in didn't complete — try signing in again.".into());
        };
        if let Some(expect) = expect_account {
            if sapisid != expect {
                return Err("The active account changed — skipping this sign-in.".into());
            }
        }
        let Some(account_key) = crate::db::account_key(&cookie) else {
            return Err("Sign-in didn't complete — try signing in again.".into());
        };
        let it = InnerTube::new(innertube::Session::default(), None)
            .map_err(|e| format!("Sign-in failed: {e}"))?;
        // No `auth` lock needed here: this `AccountSession` is a fresh, not-yet-shared local value
        // until this function returns it — nothing else could be racing on it. The lock exists for
        // the methods below that run against an *already-published* `&self` session.
        let session = AccountSession { it, account_key, auth: tokio::sync::Mutex::default() };

        // A persisted choice for THIS account (if this cookie belongs to one we've seen before)
        // takes priority over whatever channel Google defaults to today.
        let persisted_identity = selected_identity_from_db(&shared.db, &session.account_key);
        let persisted_id = persisted_identity
            .as_ref()
            .and_then(|identity| identity.data_sync_id.clone())
            .or_else(|| {
                shared.db.get_setting(&session.account_key, "data_sync_id").filter(|id| !id.is_empty())
            });

        session.it.set_cookie(Some(cookie.clone()));
        let client =
            shared.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let active = match session.it.account_menu(client).await {
            Ok(i) if i.name.is_some() => i,
            Ok(_) => return Err("That session didn't authenticate — sign in again.".into()),
            Err(e) => return Err(format!("Sign-in failed: {e}")),
        };

        let active_visitor_data = active.visitor_data.clone();
        if active_visitor_data.is_some() {
            session.it.set_visitor_data(active_visitor_data.clone());
        }
        let identities = match session.it.account_identities(client).await {
            Ok(identities) => identities,
            Err(error) => {
                tracing::warn!(%error, "could not discover alternate YouTube identities");
                Vec::new()
            }
        };

        let chosen = persisted_id
            .as_deref()
            .and_then(|id| identities.iter().find(|identity| identity.data_sync_id == id));
        if let Some(identity) = chosen {
            if identities.len() == 1 {
                let selected = SelectedIdentity::from_identity(identity, active, false);
                session.persist_selected_identity(shared, selected, active_visitor_data.as_deref())?;
                return Ok((session, SignInOutcome::Complete));
            }
            session
                .activate_identity(shared, identity, identities.len() > 1, client)
                .await?;
            return Ok((session, SignInOutcome::Complete));
        }

        if identities.is_empty() {
            if let Some(data_sync_id) = persisted_id.as_deref() {
                let identity = persisted_identity
                    .as_ref()
                    .and_then(SelectedIdentity::as_account_identity)
                    .unwrap_or_else(|| AccountIdentity {
                        name: String::new(),
                        handle: None,
                        email: None,
                        thumbnail: None,
                        channel_id: None,
                        data_sync_id: data_sync_id.to_owned(),
                        is_selected: false,
                    });
                session
                    .activate_identity(
                        shared,
                        &identity,
                        persisted_identity
                            .as_ref()
                            .is_some_and(|saved| saved.has_multiple_identities),
                        client,
                    )
                    .await?;
                return Ok((session, SignInOutcome::Complete));
            }
        }

        if identities.len() == 1 {
            let selected = SelectedIdentity::from_account_info(active, false)
                .ok_or("That session didn't return an account identity")?;
            session.persist_selected_identity(shared, selected, active_visitor_data.as_deref())?;
            return Ok((session, SignInOutcome::Complete));
        }

        if identities.len() > 1 {
            // No persisted choice for these cookies: atomically keep the authenticated cookie,
            // mark the login unfinished, and remove stale projections. A restart will reopen the
            // required picker instead of silently acting as YouTube's default channel.
            session.it.set_data_sync_id(None);
            shared
                .db
                .set_pending_auth_selection(&session.account_key, &cookie)
                .map_err(|e| e.to_string())?;
            session.persist_visitor_data(shared, active_visitor_data.as_deref());
            session.sync_active_account(shared);
            let _ = shared.events.emit("account-selection-required", ());
            return Ok((session, SignInOutcome::SelectionRequired));
        }

        // No selectable token was exposed (the normal historical single-channel response). Keep
        // the seamless path and persist the active header + response-context dataSyncId together.
        let selected = SelectedIdentity::from_account_info(active, false)
            .ok_or("That session didn't return an account identity")?;
        session.persist_selected_identity(shared, selected, active_visitor_data.as_deref())?;
        Ok((session, SignInOutcome::Complete))
    }

    /// Fresh switcher rows for the account picker. Raw delegated ids never cross the HTTP
    /// boundary; the UI receives a process-local opaque selector and display metadata only.
    pub async fn account_identities(&self, shared: &Shared) -> Result<Vec<serde_json::Value>, String> {
        if !self.it.is_logged_in() {
            return Err("Sign in before switching channels.".into());
        }
        let client =
            shared.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let identities = self.it.account_identities(client).await.map_err(|e| e.to_string())?;
        let selected_id = self.it.data_sync_id();
        let pending = auth_selection_pending(&shared.db, &self.account_key);
        Ok(identities
            .iter()
            .map(|identity| {
                let selected = !pending
                    && selected_id
                        .as_deref()
                        .map(|id| id == identity.data_sync_id)
                        .unwrap_or(identity.is_selected);
                identity_snapshot(identity, selected)
            })
            .collect())
    }

    /// Switch without a Google re-login, between channels *of this same account*. The selector is
    /// resolved against a fresh server list and account_menu must succeed under a one-off context
    /// for that exact identity before this account's transport or persistence is updated.
    pub async fn switch_account(
        &self,
        shared: &Shared,
        selection_key: &str,
    ) -> Result<serde_json::Value, String> {
        innertube::without_healing(self.switch_account_inner(shared, selection_key)).await
    }

    async fn switch_account_inner(
        &self,
        shared: &Shared,
        selection_key: &str,
    ) -> Result<serde_json::Value, String> {
        let _turn = self.auth.lock().await;
        if !self.it.is_logged_in() {
            return Err("Sign in before switching channels.".into());
        }
        let client =
            shared.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let identities = self.it.account_identities(client).await.map_err(|e| e.to_string())?;
        let identity = identities
            .iter()
            .find(|identity| identity_selection_key(&identity.data_sync_id) == selection_key)
            .ok_or(
                "That YouTube channel is no longer available. Refresh the list and try again.",
            )?;
        self.activate_identity(shared, identity, identities.len() > 1, client).await
    }

    async fn activate_identity(
        &self,
        shared: &Shared,
        identity: &AccountIdentity,
        has_multiple_identities: bool,
        client: &innertube::YouTubeClient,
    ) -> Result<serde_json::Value, String> {
        let refreshed =
            match self.it.account_menu_for_identity(client, &identity.data_sync_id).await {
                Ok(info) if info.name.is_some() => info,
                Ok(_) => {
                    return Err(
                        "YouTube did not confirm that channel. Try signing in again.".into()
                    );
                }
                Err(error) => {
                    return Err(format!("Couldn't switch YouTube channel: {error}"));
                }
            };
        let visitor_data = refreshed.visitor_data.clone();
        let selected =
            SelectedIdentity::from_identity(identity, refreshed, has_multiple_identities);
        self.persist_selected_identity(shared, selected, visitor_data.as_deref())
    }

    /// Persist a freshly picked channel: the canonical selected identity, its projections, and the
    /// visitorData, then refresh the saved account row and the transport to match. context/15.
    fn persist_selected_identity(
        &self,
        shared: &Shared,
        selected: SelectedIdentity,
        visitor_data: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let account = selected.account_json(true);
        let selected_json = serde_json::to_string(&selected).map_err(|e| e.to_string())?;
        let account_json = account.to_string();
        let session_cookie = self
            .it
            .cookie()
            .ok_or("The YouTube session expired before the channel could be saved.")?;
        shared
            .db
            .set_auth_identity(
                &self.account_key,
                &session_cookie,
                &selected_json,
                selected.data_sync_id.as_deref(),
                &account_json,
            )
            .map_err(|e| format!("Couldn't save the selected YouTube channel: {e}"))?;
        self.persist_visitor_data(shared, visitor_data);
        self.sync_active_account(shared);
        self.forget_playlist_index(shared);
        self.it.set_data_sync_id(selected.data_sync_id.clone());
        let _ = shared.events.emit("auth-changed", &account);
        Ok(account)
    }

    /// The playlist membership index belongs to one account, so a sign-out or a channel switch
    /// empties it. Its timestamp goes too, or the next sync would think it was still fresh.
    fn forget_playlist_index(&self, shared: &Shared) {
        shared.db.clear_playlist_index();
        shared.db.delete_setting(&self.account_key, "playlist_index_synced_at");
    }

    fn persist_visitor_data(&self, shared: &Shared, visitor_data: Option<&str>) {
        if let Some(visitor_data) = visitor_data {
            self.it.set_visitor_data(Some(visitor_data.to_owned()));
            shared.db.set_setting(&self.account_key, "visitor_data", visitor_data);
        }
    }

    /// Sign out: drop to guest. The saved row stays, so the account is still listed and one click
    /// from coming back.
    pub async fn sign_out(&self, shared: &Shared) {
        let _turn = self.auth.lock().await;
        self.clear_session(shared);
    }

    fn clear_session(&self, shared: &Shared) {
        self.it.set_cookie(None);
        self.it.set_data_sync_id(None);
        shared.db.delete_setting(&self.account_key, "session_cookie");
        self.forget_playlist_index(shared);
        let _ = shared.db.clear_auth_identity(&self.account_key);
        let _ = shared.events.emit("auth-changed", serde_json::json!({ "signedIn": false }));
    }

    /// Saved Google accounts for the account menu — narrowed out of this change (openspec/changes/
    /// multi-user-isolation design.md's Non-Goals): today's implementation mutates this account's
    /// live session in place to become a *different* saved account, which isn't safe once another
    /// session might reference related account state concurrently. Returns a clear error instead
    /// of silently reproducing that unsafe behavior; a later change does the real redesign
    /// ("look up or create a different AccountSession, repoint only the calling session at it").
    pub fn google_accounts(&self) -> Vec<serde_json::Value> {
        Vec::new()
    }

    pub async fn switch_google_account(&self, _id: &str) -> Result<serde_json::Value, String> {
        Err("Switching to a different saved account isn't supported yet on the web build — \
             sign out and sign back in with that account's cookie instead."
            .into())
    }

    pub async fn remove_google_account(&self, _id: &str) {
        // No-op, matching switch_google_account's narrowing: nothing here can safely mutate a
        // saved row's relationship to a *live, possibly-shared* AccountSession yet.
    }

    /// Mirror the live session + identity projections into the `accounts` table. Called at the end
    /// of every persist path (single-channel sign-in, channel pick, pending multi-channel sign-in),
    /// so the saved row for this account is always current. Best-effort: a failure leaves the
    /// projections consistent with the transport and the next persist retries.
    fn sync_active_account(&self, shared: &Shared) {
        let Some(cookie) = self.it.cookie() else { return };
        let account = StoredAccount {
            id: self.account_key.clone(),
            session_cookie: cookie.clone(),
            data_sync_id: shared
                .db
                .get_setting(&self.account_key, "data_sync_id")
                .filter(|id| !id.is_empty()),
            selected_identity_json: shared.db.get_setting(&self.account_key, "selected_identity_json"),
            account_json: shared.db.get_setting(&self.account_key, "account_json"),
            visitor_data: shared.db.get_setting(&self.account_key, "visitor_data"),
            added_at: now_secs(),
        };
        if let Err(error) = shared.db.upsert_account(&account) {
            tracing::warn!(%error, "could not persist the signed-in account row");
            return;
        }
        // The startup bootstrap reads this projection; keep it in step with the jar.
        shared.db.set_setting(&self.account_key, "session_cookie", &cookie);
    }

    /// Write a jar the transport rotated (`cookie_changed`, #165 / KI-2) back where the next
    /// launch reads it: the `session_cookie` projection and the account's own row.
    pub fn persist_rotated_cookie(&self, shared: &Shared) {
        let Some(cookie) = self.it.cookie() else { return };
        shared.db.set_setting(&self.account_key, "session_cookie", &cookie);
        shared.db.update_account_cookie(&self.account_key, &cookie);
    }

    /// Keep-alive ping: rolls the session tokens and returns a fresh login-bound visitorData.
    /// The transport's own rotation only fires on requests the app makes, so an idle night leaves
    /// the jar to go stale.
    pub async fn keep_session_alive(&self, shared: &Shared) {
        if self.it.cookie().is_none() {
            return;
        }
        let Some(client) = shared.clients.get(innertube::METADATA_CLIENT) else { return };
        match self.it.account_menu(client).await {
            Ok(info) if info.name.is_some() => {
                self.persist_visitor_data(shared, info.visitor_data.as_deref());
                self.sync_active_account(shared);
                tracing::debug!(account = %self.account_key, "session keep-alive successful");
            }
            Ok(_) => tracing::warn!("session keep-alive: the session did not authenticate"),
            Err(error) => tracing::warn!(%error, "session keep-alive failed"),
        }
    }

    /// Current account for the UI. New installs derive it from the canonical selected identity;
    /// legacy `account_json` remains a read fallback so existing databases migrate without a wipe.
    pub fn account_snapshot(&self, shared: &Shared) -> serde_json::Value {
        if auth_selection_pending(&shared.db, &self.account_key) && self.it.is_logged_in() {
            return serde_json::json!({
                "signedIn": true,
                "selectionRequired": true,
                "canSwitch": true,
            });
        }
        if let Some(selected) = selected_identity_from_db(&shared.db, &self.account_key) {
            return selected.account_json(self.it.is_logged_in());
        }
        let mut v = shared
            .db
            .get_setting(&self.account_key, "account_json")
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        v["signedIn"] = serde_json::json!(self.it.is_logged_in());
        v
    }
}

/// An anonymous caller's settings scope — never a real `account_key`, so it never collides with
/// one (every real one is the `ga-<md5>` shape `account_key()` produces).
pub const ANONYMOUS: &str = GLOBAL_ACCOUNT;

/// A signed-in request's view of both halves of the split (openspec/changes/multi-user-isolation):
/// the process-wide `Shared` plus this one request's own `AccountSession`. Replaces `&AppState` in
/// every `commands/*.rs` signature — the field-access paths below (`.db()`, `.it()`,
/// `.account_key()`) keep those call sites close to their pre-split shape instead of threading two
/// separate parameters through every command function.
pub struct Scoped<'a> {
    pub shared: &'a Shared,
    pub account: &'a AccountSession,
}

impl<'a> Scoped<'a> {
    pub fn db(&self) -> &Db {
        &self.shared.db
    }
    pub fn it(&self) -> &InnerTube {
        &self.account.it
    }
    pub fn account_key(&self) -> &str {
        &self.account.account_key
    }
    pub fn clients(&self) -> &Clients {
        &self.shared.clients
    }
}

/// `Scoped`'s owned, `Clone`able counterpart, for the one caller (`lastfm::connect`) that spawns a
/// background task outliving the request and needs to move its scope into it rather than borrow.
#[derive(Clone)]
pub struct ScopedOwned {
    pub shared: Arc<Shared>,
    pub account: Arc<AccountSession>,
}

impl ScopedOwned {
    pub fn db(&self) -> &Db {
        &self.shared.db
    }
    pub fn account_key(&self) -> &str {
        &self.account.account_key
    }
}
