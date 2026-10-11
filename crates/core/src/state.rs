//! Account/session state, trimmed from src-tauri's `AppState` (openspec/changes/
//! signed-in-http-parity). context/11.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use std::sync::Arc;

use innertube::{AccountIdentity, AccountInfo, AudioQuality, Clients, InnerTube};

use crate::db::{now_secs, Db, StoredAccount};

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

// Trimmed fork of src-tauri's `AppState` (openspec/changes/signed-in-http-parity design.md):
// dropped `player`/`app`/`orchestrator`/`lt`/`cache_dir`/`media`/`discord`/`rate_epoch` and every
// playback/queue/OS-media-control field (`queue`, `generation`, `pending_seek`, `history_pinged`,
// `is_playing`, `latest_position`, `last_pos_persist`, `last_media_push`, `last_media_state`,
// `video_urls`, `last_queue_fingerprint`, `last_persisted_fingerprint`) — none reachable from any
// capability this crate serves.
pub struct AppState {
    pub it: InnerTube,
    pub clients: Clients,
    pub db: Arc<Db>,
    /// Replaces `AppHandle::emit` — see `crate::events`.
    pub events: crate::events::Broadcaster,
    /// Last.fm scrobbler. Same feed again; parks until a session key is set (titlebar button).
    pub lastfm: crate::lastfm::LastfmHandle,
    /// Stream resolution (cipher/PoToken/client-fallback pipeline) — re-added by
    /// openspec/changes/http-audio-streaming, reversing this file's own note above that dropped it
    /// (it "only matter[s] once streaming exists").
    pub orchestrator: Arc<crate::media::orchestrator::Orchestrator>,
    /// One authentication transition at a time: [`Self::sign_in`], [`Self::switch_account`],
    /// [`Self::sign_out`]. Each reads the cookie and identity, awaits YouTube, then writes both to
    /// the transport and the database, so two of them overlapping lets the loser land last and
    /// win: a failed heal restoring its stale snapshot over a fresh login, or a heal completing
    /// after the user signed out and putting them back in. Async because it is held across those
    /// awaits, which a std `Mutex` cannot be.
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

fn selected_identity_from_db(db: &Db) -> Option<SelectedIdentity> {
    db.get_setting("selected_identity_json")
        .and_then(|json| serde_json::from_str::<SelectedIdentity>(&json).ok())
        .filter(|identity| identity.name.is_some() || identity.data_sync_id.is_some())
        .or_else(|| {
            let account = db
                .get_setting("account_json")
                .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())?;
            let string = |key: &str| {
                account
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            Some(SelectedIdentity {
                data_sync_id: db.get_setting("data_sync_id").filter(|id| !id.is_empty()),
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

fn auth_selection_pending(db: &Db) -> bool {
    db.get_setting("account_selection_pending").as_deref() == Some("true")
}

/// Startup compatibility: prefer the canonical model, then fall back to the pre-switcher setting.
pub(crate) fn persisted_data_sync_id(db: &Db) -> Option<String> {
    selected_identity_from_db(db)
        .and_then(|identity| identity.data_sync_id)
        .or_else(|| db.get_setting("data_sync_id").filter(|id| !id.is_empty()))
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
impl AppState {
    /// `app_data_dir` is where the cipher caches `player.js` and its config registry snapshot
    /// (`crates/core/src/cipher/{fetcher,config}.rs`) — the server's equivalent of the desktop
    /// app's app-data directory, not reused for anything else here.
    pub fn new(
        it: InnerTube,
        clients: Clients,
        db: Arc<Db>,
        lastfm: crate::lastfm::LastfmHandle,
        app_data_dir: &std::path::Path,
    ) -> Self {
        let config = Arc::new(crate::cipher::PlayerConfigStore::new(app_data_dir));
        let cipher = Arc::new(crate::cipher::CipherDeobfuscator::new(app_data_dir, config));
        let potoken = Arc::new(crate::potoken::PoTokenGenerator::new(db.clone()));
        let orchestrator = Arc::new(crate::media::orchestrator::Orchestrator::new(
            it.clone(),
            clients.clone(),
            cipher,
            potoken,
        ));
        AppState {
            it,
            clients,
            db,
            events: crate::events::Broadcaster::default(),
            lastfm,
            orchestrator,
            auth: tokio::sync::Mutex::default(),
        }
    }

    // --- stream resolution (openspec/changes/http-audio-streaming) --------------------------

    /// The stored `quality` setting, same mapping as the desktop app's own `AppState::quality`.
    pub fn quality(&self) -> AudioQuality {
        match self.db.get_setting("quality").as_deref() {
            Some("LOW") => AudioQuality::Low,
            Some("AUTO") => AudioQuality::Auto,
            _ => AudioQuality::High,
        }
    }

    /// User-disabled stream clients — same comma-separated `disabled_stream_clients` setting and
    /// `LIMUSIC_DISABLED_CLIENTS` env override as the desktop app's own `AppState::disabled_clients`.
    pub fn disabled_clients(&self) -> HashSet<String> {
        let raw = std::env::var("LIMUSIC_DISABLED_CLIENTS")
            .ok()
            .or_else(|| self.db.get_setting("disabled_stream_clients"))
            .unwrap_or_default();
        raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    }

    // --- auth (context/15) ------------------------------------------------------------------

    /// Sign in with the Cookie header captured from the login webview (context/15 Path A).
    ///
    /// `account_menu` validates the cookie and provides the active header. `accounts_list` then
    /// discovers every server-selectable identity. A persisted matching identity wins over
    /// Google's current default; a new multi-channel login pauses before finalization and asks the
    /// UI to choose.
    /// `expect_account` is the SAPISID the caller read before it went off to fetch `cookie`. The
    /// session heal needs it: the user can switch accounts while it exports the jar, and signing
    /// in with what it found would drag them back to the account they just left.
    pub async fn sign_in(
        &self,
        cookie: String,
        expect_account: Option<&str>,
    ) -> Result<SignInOutcome, String> {
        // Validating a cookie already in hand: a 401 here is the answer, not something to wait on
        // a heal for. `refresh_session` also reaches `sign_in` while healing, and a wait there
        // would park the healing task on the heal it is performing, holding `auth` for the whole
        // timeout.
        innertube::without_healing(self.sign_in_inner(cookie, expect_account)).await
    }

    async fn sign_in_inner(
        &self,
        cookie: String,
        expect_account: Option<&str>,
    ) -> Result<SignInOutcome, String> {
        let _turn = self.auth.lock().await;
        if let Some(expect) = expect_account {
            let live = self.it.cookie().unwrap_or_default();
            if innertube::cookie_sapisid(&live) != Some(expect) {
                return Err("The active account changed — skipping this sign-in.".into());
            }
        }
        let cookie = cookie.trim().to_owned();
        if innertube::cookie_sapisid(&cookie).is_none() {
            return Err("Sign-in didn't complete — try signing in again.".into());
        }
        let previous_cookie = self.it.cookie();
        let previous_data_sync_id = self.it.data_sync_id();
        let persisted_identity = selected_identity_from_db(&self.db);
        let persisted_id = persisted_identity
            .as_ref()
            .and_then(|identity| identity.data_sync_id.clone())
            .or_else(|| self.db.get_setting("data_sync_id").filter(|id| !id.is_empty()));

        self.it.set_cookie(Some(cookie.clone()));
        // Discovery must not inherit a stale/default identity. The persisted id is restored only
        // after the fresh accounts list proves that this cookie can act as it.
        self.it.set_data_sync_id(None);
        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let active = match self.it.account_menu(client).await {
            // A valid, authenticating cookie returns the account header (name). No name means the
            // session didn't actually authenticate — reject it up front so we don't "succeed" into
            // a silently-empty library.
            Ok(i) if i.name.is_some() => i,
            Ok(_) => {
                self.restore_auth_transport(previous_cookie, previous_data_sync_id);
                return Err("That session didn't authenticate — sign in again.".into());
            }
            // Auth didn't take (network) — roll back so we're not half-logged-in.
            Err(e) => {
                self.restore_auth_transport(previous_cookie, previous_data_sync_id);
                return Err(format!("Sign-in failed: {e}"));
            }
        };

        let active_visitor_data = active.visitor_data.clone();

        // Discovery must go out with the login-bound visitorData account_menu just handed us, not
        // the anonymous bootstrap id still sitting in the session: pairing that id with login
        // cookies is what gets accounts_list rejected, and a rejected list reads below as "this
        // Google account has one channel". Every success path persists this same value; hand the
        // old one back until then so a rolled-back sign-in isn't left holding it.
        let previous_visitor_data = self.it.visitor_data();
        if active_visitor_data.is_some() {
            self.it.set_visitor_data(active_visitor_data.clone());
        }
        // Losing the list endpoint must not regress ordinary one-channel sign-in. It only removes
        // the optional switcher for this login attempt; account_menu still supplies a valid active
        // identity exactly as older releases used it.
        let identities = match self.it.account_identities(client).await {
            Ok(identities) => identities,
            Err(error) => {
                tracing::warn!(%error, "could not discover alternate YouTube identities");
                Vec::new()
            }
        };
        self.it.set_visitor_data(previous_visitor_data);

        let chosen = persisted_id
            .as_deref()
            .and_then(|id| identities.iter().find(|identity| identity.data_sync_id == id));
        if let Some(identity) = chosen {
            if identities.len() == 1 {
                let selected = SelectedIdentity::from_identity(identity, active, false);
                self.persist_selected_identity(selected, active_visitor_data.as_deref())
                    .inspect_err(|_| {
                        self.restore_auth_transport(
                            previous_cookie.clone(),
                            previous_data_sync_id.clone(),
                        );
                        self.restore_session_cookie_setting(previous_cookie.as_deref());
                    })?;
                return Ok(SignInOutcome::Complete);
            }
            self.activate_identity(identity, identities.len() > 1, client).await.inspect_err(
                |_| {
                    self.restore_auth_transport(
                        previous_cookie.clone(),
                        previous_data_sync_id.clone(),
                    );
                    self.restore_session_cookie_setting(previous_cookie.as_deref());
                },
            )?;
            return Ok(SignInOutcome::Complete);
        }

        // A missing/unreadable list must not replace a previously selected channel with Google's
        // current default. Revalidate the stored server-issued id directly; account_menu either
        // confirms it and refreshes metadata, or the login fails closed.
        //
        // Only when the list is empty. A list that came back and does *not* contain the persisted
        // id means these cookies belong to a different Google account, so the stored id is theirs
        // to drop: forcing it here would delegate account A's channel onto account B's cookie and
        // fail every sign-in until the user signs out first.
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
                self.activate_identity(
                    &identity,
                    persisted_identity.as_ref().is_some_and(|saved| saved.has_multiple_identities),
                    client,
                )
                .await
                .inspect_err(|_| {
                    self.restore_auth_transport(
                        previous_cookie.clone(),
                        previous_data_sync_id.clone(),
                    );
                    self.restore_session_cookie_setting(previous_cookie.as_deref());
                })?;
                return Ok(SignInOutcome::Complete);
            }
        }

        if identities.len() == 1 {
            // One channel stays seamless. Reuse the already-fetched active header instead of
            // introducing another network request or a new delegated id into the historical
            // single-account path.
            let selected = SelectedIdentity::from_account_info(active, false)
                .ok_or("That session didn't return an account identity")?;
            self.persist_selected_identity(selected, active_visitor_data.as_deref()).inspect_err(
                |_| {
                    self.restore_auth_transport(
                        previous_cookie.clone(),
                        previous_data_sync_id.clone(),
                    );
                    self.restore_session_cookie_setting(previous_cookie.as_deref());
                },
            )?;
            return Ok(SignInOutcome::Complete);
        }

        if identities.len() > 1 {
            // No persisted choice for these cookies: atomically keep the authenticated cookie,
            // mark the login unfinished, and remove stale projections. A restart will reopen the
            // required picker instead of silently acting as YouTube's default channel.
            self.it.set_data_sync_id(None);
            self.db.set_pending_auth_selection(&cookie, None).map_err(|error| {
                self.restore_auth_transport(previous_cookie.clone(), previous_data_sync_id.clone());
                self.restore_session_cookie_setting(previous_cookie.as_deref());
                error.to_string()
            })?;
            self.persist_visitor_data(active_visitor_data.as_deref());
            self.sync_active_account();
            let _ = self.events.emit("account-selection-required", ());
            return Ok(SignInOutcome::SelectionRequired);
        }

        // No selectable token was exposed (the normal historical single-channel response). Keep
        // the seamless path and persist the active header + response-context dataSyncId together.
        let selected = SelectedIdentity::from_account_info(active, false)
            .ok_or("That session didn't return an account identity")?;
        self.persist_selected_identity(selected, active_visitor_data.as_deref()).inspect_err(
            |_| {
                self.restore_auth_transport(previous_cookie.clone(), previous_data_sync_id.clone());
                self.restore_session_cookie_setting(previous_cookie.as_deref());
            },
        )?;
        Ok(SignInOutcome::Complete)
    }

    /// Fresh switcher rows for the account picker. Raw delegated ids never cross the Tauri
    /// boundary; the UI receives a process-local opaque selector and display metadata only.
    pub async fn account_identities(&self) -> Result<Vec<serde_json::Value>, String> {
        if !self.it.is_logged_in() {
            return Err("Sign in before switching channels.".into());
        }
        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let identities = self.it.account_identities(client).await.map_err(|e| e.to_string())?;
        let selected_id = self.it.data_sync_id();
        // Nothing is selected yet during a forced first pick, so YouTube's own default marker
        // would be a lie: the user hasn't chosen it, and picking it is still a required step.
        let pending = auth_selection_pending(&self.db);
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

    /// Switch without a Google re-login. The selector is resolved against a fresh server list and
    /// account_menu must succeed under a one-off context for that exact identity before the shared
    /// transport, persistence, or UI is updated.
    pub async fn switch_account(&self, selection_key: &str) -> Result<serde_json::Value, String> {
        // Same as `sign_in`: this validates an identity it was handed, so let it hear the answer.
        innertube::without_healing(self.switch_account_inner(selection_key)).await
    }

    async fn switch_account_inner(&self, selection_key: &str) -> Result<serde_json::Value, String> {
        let _turn = self.auth.lock().await;
        if !self.it.is_logged_in() {
            return Err("Sign in before switching channels.".into());
        }
        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let identities = self.it.account_identities(client).await.map_err(|e| e.to_string())?;
        let identity = identities
            .iter()
            .find(|identity| identity_selection_key(&identity.data_sync_id) == selection_key)
            .ok_or(
                "That YouTube channel is no longer available. Refresh the list and try again.",
            )?;
        self.activate_identity(identity, identities.len() > 1, client).await
    }

    async fn activate_identity(
        &self,
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
        self.persist_selected_identity(selected, visitor_data.as_deref())
    }

    /// Persist a freshly picked channel: the canonical selected identity, its projections, and the
    /// visitorData, then refresh the saved account row and the transport to match. context/15.
    fn persist_selected_identity(
        &self,
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
        self.db
            .set_auth_identity(
                &session_cookie,
                &selected_json,
                selected.data_sync_id.as_deref(),
                &account_json,
            )
            .map_err(|e| format!("Couldn't save the selected YouTube channel: {e}"))?;
        self.persist_visitor_data(visitor_data);
        self.sync_active_account();
        self.forget_playlist_index();
        self.it.set_data_sync_id(selected.data_sync_id.clone());
        let _ = self.events.emit("auth-changed", &account);
        Ok(account)
    }

    /// The playlist membership index belongs to one account, so a sign-out or a channel switch
    /// empties it. Its timestamp goes too, or the next sync would think it was still fresh.
    fn forget_playlist_index(&self) {
        self.db.clear_playlist_index();
        self.db.delete_setting("playlist_index_synced_at");
    }

    fn restore_auth_transport(&self, cookie: Option<String>, data_sync_id: Option<String>) {
        self.it.set_cookie(cookie);
        self.it.set_data_sync_id(data_sync_id);
    }

    fn restore_session_cookie_setting(&self, cookie: Option<&str>) {
        if let Some(cookie) = cookie {
            self.db.set_setting("session_cookie", cookie);
        } else {
            self.db.delete_setting("session_cookie");
        }
    }

    fn persist_visitor_data(&self, visitor_data: Option<&str>) {
        if let Some(visitor_data) = visitor_data {
            self.it.set_visitor_data(Some(visitor_data.to_owned()));
            self.db.set_setting("visitor_data", visitor_data);
        }
    }

    /// Sign out: drop to guest. The saved row stays, so the account is still listed and one click
    /// from coming back. Deleting the credentials is `remove_google_account`, which the menu offers
    /// as its own action behind a confirm.
    pub async fn sign_out(&self) {
        let _turn = self.auth.lock().await;
        self.clear_session();
    }

    /// Saved Google accounts for the account menu. Display fields only: cookies, delegated ids and
    /// visitorData never cross the Tauri boundary (context/15).
    pub fn google_accounts(&self) -> Vec<serde_json::Value> {
        let active = self.db.get_setting("active_account");
        self.db
            .list_accounts()
            .into_iter()
            .map(|account| {
                let info = account
                    .account_json
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                    .unwrap_or_else(|| serde_json::json!({}));
                serde_json::json!({
                    "id": account.id,
                    "name": info.get("name").cloned(),
                    "handle": info.get("handle").cloned(),
                    "email": info.get("email").cloned(),
                    "thumbnail": info.get("thumbnail").cloned(),
                    "active": active.as_deref() == Some(account.id.as_str()),
                })
            })
            .collect()
    }

    /// Activate a saved Google account without a webview round trip. The stored cookie is
    /// validated with `account_menu` first: Google rotates its short-lived tokens, and activating
    /// a dead session would leave the app half-logged-in. On success the transport, the
    /// projections and the UI all move together; on failure the previous session is left untouched.
    pub async fn switch_google_account(&self, id: &str) -> Result<serde_json::Value, String> {
        // The stored cookie being stale is the expected failure here, and the caller rolls back
        // on it. Waiting on a heal would only delay that by up to `HEAL_TIMEOUT`.
        innertube::without_healing(self.switch_google_account_inner(id)).await
    }

    async fn switch_google_account_inner(&self, id: &str) -> Result<serde_json::Value, String> {
        let _turn = self.auth.lock().await;
        let account = self.db.get_account(id).ok_or("That account is no longer saved.")?;
        let previous_cookie = self.it.cookie();
        let previous_data_sync_id = self.it.data_sync_id();
        let previous_visitor = self.it.visitor_data();

        self.it.set_cookie(Some(account.session_cookie.clone()));
        self.it.set_data_sync_id(account.data_sync_id.clone());
        self.it.set_visitor_data(account.visitor_data.clone());

        // An account saved mid multi-channel sign-in (cookie authenticated, no channel picked
        // yet) returns to the required picker instead of silently acting as YouTube's default.
        if account.selected_identity_json.is_none() {
            // Same rollback as the `account_menu` failure below: the transport already carries the
            // selected account, so returning without undoing it would leave the app requesting as
            // one account while the database and the UI still describe the other.
            if let Err(e) = self.db.set_pending_auth_selection(&account.session_cookie, Some(id)) {
                self.restore_auth_transport(previous_cookie, previous_data_sync_id);
                self.it.set_visitor_data(previous_visitor);
                return Err(format!("Couldn't activate the account: {e}"));
            }
            self.persist_visitor_data(account.visitor_data.as_deref());
            self.forget_playlist_index();
            let snapshot = self.account_snapshot();
            let _ = self.events.emit("account-selection-required", ());
            let _ = self.events.emit("auth-changed", &snapshot);
            return Ok(snapshot);
        }

        let client =
            self.clients.get(innertube::METADATA_CLIENT).ok_or("metadata client missing")?;
        let outcome = match self.it.account_menu(client).await {
            Ok(info) if info.name.is_some() => Ok(info.visitor_data),
            Ok(_) => Err("That account's session has expired. Sign in with it again.".to_owned()),
            Err(error) => Err(format!("Couldn't switch accounts: {error}")),
        };
        let refreshed_visitor = match outcome {
            Ok(visitor_data) => visitor_data,
            Err(message) => {
                self.restore_auth_transport(previous_cookie, previous_data_sync_id);
                self.it.set_visitor_data(previous_visitor);
                return Err(message);
            }
        };

        self.db.restore_account(&account).map_err(|e| format!("Couldn't switch accounts: {e}"))?;
        // `account_menu` hands back a login-bound visitorData, and it may already have rotated the
        // jar out from under the row `restore_account` just wrote back. Re-sync both from what the
        // transport actually holds now.
        self.persist_visitor_data(refreshed_visitor.as_deref());
        self.sync_active_account();
        self.forget_playlist_index();
        let snapshot = self.account_snapshot();
        let _ = self.events.emit("auth-changed", &snapshot);
        Ok(snapshot)
    }

    /// Remove a saved Google account from the menu. Removing the active one signs out too.
    pub async fn remove_google_account(&self, id: &str) {
        let _turn = self.auth.lock().await;
        self.forget_account(id);
    }

    /// Delete a saved row, dropping to guest when it is the one driving requests. Active-ness
    /// comes from the live session cookie first, with `active_account` as the fallback for an
    /// already signed-out transport, so removing an account can never leave the transport holding
    /// its credentials.
    fn forget_account(&self, id: &str) {
        let live = self.it.cookie().and_then(|cookie| crate::db::account_key(&cookie));
        let active = live.as_deref() == Some(id)
            || self.db.get_setting("active_account").as_deref() == Some(id);
        self.db.remove_account(id);
        if active {
            self.clear_session();
        }
    }

    /// Drop the live session back to guest, leaving the saved rows alone.
    fn clear_session(&self) {
        self.it.set_cookie(None);
        self.it.set_data_sync_id(None);
        self.db.delete_setting("session_cookie");
        self.db.delete_setting("active_account");
        self.forget_playlist_index();
        let _ = self.db.clear_auth_identity();
        let _ = self.events.emit("auth-changed", serde_json::json!({ "signedIn": false }));
    }

    /// Mirror the live session + identity projections into the `accounts` table. Called at the end
    /// of every persist path (single-channel sign-in, channel pick, pending multi-channel sign-in,
    /// account switch), so the saved row for the active session is always current. Best-effort: a
    /// failure leaves the projections consistent with the transport and the next persist retries.
    fn sync_active_account(&self) {
        let Some(cookie) = self.it.cookie() else { return };
        let Some(id) = crate::db::account_key(&cookie) else { return };
        let account = StoredAccount {
            id,
            session_cookie: cookie.clone(),
            data_sync_id: self.db.get_setting("data_sync_id").filter(|id| !id.is_empty()),
            selected_identity_json: self.db.get_setting("selected_identity_json"),
            account_json: self.db.get_setting("account_json"),
            visitor_data: self.db.get_setting("visitor_data"),
            added_at: now_secs(),
        };
        if let Err(error) = self.db.upsert_account(&account) {
            tracing::warn!(%error, "could not persist the signed-in account row");
            return;
        }
        self.db.set_setting("active_account", &account.id);
        // The startup bootstrap reads this projection; keep it in step with the jar.
        self.db.set_setting("session_cookie", &cookie);
    }

    /// Write a jar the transport rotated (`cookie_changed`, #165 / KI-2) back where the next
    /// launch and the next account switch read it: the `session_cookie` projection and the
    /// account's own row. Keyed on the jar's own SAPISID, so a switch racing this write cannot
    /// file one account's cookies under another account's row.
    pub fn persist_rotated_cookie(&self) {
        let Some(cookie) = self.it.cookie() else { return };
        let Some(id) = crate::db::account_key(&cookie) else { return };
        self.db.set_setting("session_cookie", &cookie);
        self.db.update_account_cookie(&id, &cookie);
    }

    /// Keep-alive ping: rolls the session tokens and returns a fresh login-bound visitorData.
    /// The transport's own rotation only fires on requests the app makes, so an idle night leaves
    /// the jar to go stale, and `refresh_session` can only re-mint whichever account the login
    /// webview happens to hold. This is what keeps a switched-to account alive.
    ///
    /// Gated on the live cookie still being the active account, and re-checked after the await so
    /// a switch landing mid-request cannot be overwritten.
    pub async fn keep_session_alive(&self) {
        let Some(cookie) = self.it.cookie() else { return };
        let Some(active) = self.db.get_setting("active_account") else { return };
        if crate::db::account_key(&cookie).as_deref() != Some(active.as_str()) {
            return;
        }
        let Some(client) = self.clients.get(innertube::METADATA_CLIENT) else { return };
        match self.it.account_menu(client).await {
            Ok(info) if info.name.is_some() => {
                let live = self.it.cookie().and_then(|c| crate::db::account_key(&c));
                if live.as_deref() != Some(active.as_str())
                    || self.db.get_setting("active_account").as_deref() != Some(active.as_str())
                {
                    return;
                }
                self.persist_visitor_data(info.visitor_data.as_deref());
                self.sync_active_account();
                tracing::debug!("session keep-alive successful");
            }
            Ok(_) => tracing::warn!("session keep-alive: the session did not authenticate"),
            Err(error) => tracing::warn!(%error, "session keep-alive failed"),
        }
    }

    /// Current account for the UI. New installs derive it from the canonical selected identity;
    /// legacy `account_json` remains a read fallback so existing databases migrate without a wipe.
    pub fn account_snapshot(&self) -> serde_json::Value {
        if auth_selection_pending(&self.db) && self.it.is_logged_in() {
            return serde_json::json!({
                "signedIn": true,
                "selectionRequired": true,
                "canSwitch": true,
            });
        }
        if let Some(selected) = selected_identity_from_db(&self.db) {
            return selected.account_json(self.it.is_logged_in());
        }
        let mut v = self
            .db
            .get_setting("account_json")
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        v["signedIn"] = serde_json::json!(self.it.is_logged_in());
        v
    }
}

