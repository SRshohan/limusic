// The browser/PWA implementation of the UI's command surface. No Tauri: browse/search, auth,
// library, playlists, ratings, settings, lyrics, Last.fm, blocked artists, diagnostics, and Listen
// Together are all real HTTP/WebSocket calls against `crates/server` (openspec/changes/
// http-browse-vertical-slice, signed-in-http-parity). What's left stubbed is inherently
// playback/streaming (play/queue/mpv) and desktop-only OS integration (local-folder scan, app
// icon, font files) — Phase 3 and later, per signed-in-http-parity's design.md Non-Goals. Never
// imported directly outside `api.ts` — see platform.ts/api.ts for why this and `api.tauri.ts` are
// picked between at module load instead of branching per call.
import type {
	Account,
	AccountIdentity,
	AlbumPage,
	ArtistPage,
	BlockedArtist,
	BrowseItem,
	HistoryGroup,
	HomePage,
	HotkeyRegisterResult,
	HotkeysConfig,
	LastfmState,
	LocalLibrary,
	LtState,
	Lyrics,
	LyricsProvider,
	LyricsTrack,
	MoodSection,
	NowPlaying,
	PlaybackSnapshot,
	PlaylistContinuation,
	PlaylistPage,
	QueueAppended,
	QueueIndex,
	QueueState,
	Rating,
	ReleaseNote,
	RepeatMode,
	SavedAccount,
	SearchResults,
	SearchSuggestions,
	ServerSort,
	SongItem,
	UnlistenFn
} from './api.types';
import { subscribeWs } from './ws';
import * as lt from './lt-client';
import * as wp from './web-playback.svelte';

export * from './api.types';

// Dev (`pnpm dev`): vite serves the UI on :5183, the server runs separately on :8787, so requests
// need the explicit cross-origin URL. Production build: Caddy serves the built UI and proxies
// /api + /ws from the same hostname (see deploy/Caddyfile), so same-origin relative paths are
// correct and there's no CORS/cookie-SameSite concern to manage.
const SERVER_URL = import.meta.env.DEV ? 'http://localhost:8787' : '';

/** Every not-yet-ported command rejects through this, instead of the cryptic
 *  "Cannot read properties of undefined (reading 'invoke')" calling `@tauri-apps/api` outside a
 *  Tauri webview used to produce — the goal is a clear, intentional failure, not a working feature. */
const unsupported = (command: string): Promise<never> =>
	Promise.reject(new Error(`${command}: not available in the web build yet (desktop-only command)`));

/** Same idea as `unsupported`, but for an event subscription: there's nothing to fail *to*, so
 *  this resolves with an unlisten that does nothing, rather than rejecting a promise no caller
 *  expects to reject (every `onX` in the Tauri surface always resolves). */
const noSubscription = (): Promise<UnlistenFn> => Promise.resolve(() => {});

/** Shared by every ported route: `fetch` + the same non-ok-response convention `search`
 *  originally established alone (openspec/changes/http-browse-vertical-slice). `credentials:
 *  'include'` on every request (openspec/changes/signed-in-http-parity) — most routes don't need
 *  the session cookie, but the ones that do get no special-casing, and sending it costs nothing
 *  when there isn't one yet. A `204 No Content` response (every write action below that doesn't
 *  hand back a value) resolves `undefined` rather than failing to parse an empty body as JSON. */
async function request<T>(path: string, init?: RequestInit): Promise<T> {
	const res = await fetch(`${SERVER_URL}${path}`, { credentials: 'include', ...init });
	if (!res.ok) throw new Error(`${path} failed: ${res.status}`);
	if (res.status === 204) return undefined as T;
	return res.json();
}

const fetchJson = <T>(path: string): Promise<T> => request<T>(path);

const jsonBody = (method: string, body: unknown): RequestInit => ({
	method,
	headers: { 'Content-Type': 'application/json' },
	body: JSON.stringify(body)
});

const postJson = <T>(path: string, body: unknown): Promise<T> => request<T>(path, jsonBody('POST', body));
const putJson = <T>(path: string, body: unknown): Promise<T> => request<T>(path, jsonBody('PUT', body));
const patchJson = <T>(path: string, body: unknown): Promise<T> => request<T>(path, jsonBody('PATCH', body));
const deleteJson = <T>(path: string, body?: unknown): Promise<T> =>
	request<T>(path, body === undefined ? { method: 'DELETE' } : jsonBody('DELETE', body));
/** `putJson`'s counterpart for a raw-bytes body (`setPlaylistCover`'s image upload) — no
 *  `Content-Type` set, the browser's own `Blob`/`File` MIME type rides along on the request. */
const putBytes = <T>(path: string, bytes: Blob | Uint8Array | ArrayBuffer): Promise<T> =>
	request<T>(path, { method: 'PUT', body: bytes as BodyInit });

/** `fetchJson`'s counterpart for the one route that answers plain text (`/api/diagnostics*`). */
async function fetchText(path: string): Promise<string> {
	const res = await fetch(`${SERVER_URL}${path}`, { credentials: 'include' });
	if (!res.ok) throw new Error(`${path} failed: ${res.status}`);
	return res.text();
}

// --- commands (context/11) -----------------------------------------------------------------
// `recordHistory` is true only for a query the user submitted: a signed-in search is written to the
// account's YouTube search history, so a typeahead preview must stay anonymous (#203). No session
// to record it against yet on this path (see design.md) — recordHistory is silently a no-op here,
// same as it already is for an anonymous Tauri search.
export const search = (query: string, _recordHistory = false): Promise<SongItem[]> =>
	fetchJson(`/api/search?q=${encodeURIComponent(query)}`);
export const searchVideos = (query: string): Promise<SongItem[]> =>
	fetchJson(`/api/search/videos?q=${encodeURIComponent(query)}`);
export const searchAll = (query: string, _recordHistory = false): Promise<SearchResults> =>
	fetchJson(`/api/search/all?q=${encodeURIComponent(query)}`);
export const searchSuggestions = (query: string): Promise<SearchSuggestions> =>
	fetchJson(`/api/search/suggestions?q=${encodeURIComponent(query)}`);
export const searchCards = (
	query: string,
	category: 'albums' | 'artists' | 'playlists'
): Promise<BrowseItem[]> =>
	fetchJson(`/api/search/cards?q=${encodeURIComponent(query)}&category=${category}`);
// --- playback (openspec/changes/http-audio-streaming) — client-owned, see web-playback.svelte.ts
export const play = (item: SongItem): Promise<void> => wp.play(item);
export const playIndex = (index: number): Promise<void> => wp.playIndex(index);
export const removeFromQueue = (index: number): Promise<void> => {
	wp.removeFromQueue(index);
	return Promise.resolve();
};
export const moveInQueue = (from: number, to: number): Promise<void> => {
	wp.moveInQueue(from, to);
	return Promise.resolve();
};
export const playNext = (items: SongItem[], from?: string): Promise<void> => {
	wp.playNext(items, from);
	return Promise.resolve();
};
export const addToQueue = (
	items: SongItem[],
	from?: string,
	_continuation?: string
): Promise<void> => {
	wp.addToQueue(items, from);
	return Promise.resolve();
};
export const clearQueued = (): Promise<void> => {
	wp.clearQueued();
	return Promise.resolve();
};
export const nextTrack = (): Promise<void> => wp.nextTrack();
export const prevTrack = (): Promise<void> => wp.prevTrack();
export const backToPrevious = (): Promise<void> => {
	wp.backToPrevious();
	return Promise.resolve();
};
export const toggleShuffle = (): Promise<void> => {
	wp.toggleShuffle();
	return Promise.resolve();
};
export const setRepeat = (mode: RepeatMode): Promise<void> => {
	wp.setRepeat(mode);
	return Promise.resolve();
};
export const togglePause = (): Promise<void> => {
	wp.togglePause();
	return Promise.resolve();
};
export const seek = (position: number): Promise<void> => {
	wp.seek(position);
	return Promise.resolve();
};
export const setVolume = (volume: number): Promise<void> => {
	wp.setVolume(volume);
	return Promise.resolve();
};
/** Tempo/pitch has no Web Audio equivalent and is dropped entirely (design.md Non-Goals) — stays
 *  unsupported permanently, not just until a later task. */
export const setPlaybackParams = (_speed: number, _semitones: number): Promise<void> =>
	unsupported('setPlaybackParams');
export const getQueue = (): Promise<QueueState> => Promise.resolve(wp.getQueue());
/** The web build resolves video alongside audio in one call (`wp.currentVideoUrl()`); this
 *  desktop-shaped per-call proxy lookup has no web equivalent and stays unsupported. */
export const videoStream = (_videoId: string, _maxHeight: number): Promise<string | null> =>
	unsupported('videoStream');
/** No separate video-URL cache to clear on the web build (see `videoStream` above) — a harmless
 *  no-op rather than a hard failure, since callers treat this as fire-and-forget cleanup. */
export const forgetVideoStream = (_videoId: string): Promise<void> => Promise.resolve();
export const nativeVideoRect = (
	_rect: [number, number, number, number] | null
): Promise<boolean> => unsupported('nativeVideoRect');
export const ambientFrame = (_after: number): Promise<ArrayBuffer | number[]> =>
	unsupported('ambientFrame');
export const getPlayback = (): Promise<PlaybackSnapshot> => Promise.resolve(wp.getPlayback());

// --- settings (context/11) -----------------------------------------------------------------
export const getSettings = (): Promise<Record<string, string>> => fetchJson('/api/settings');
export const setSetting = (key: string, value: string): Promise<void> =>
	putJson(`/api/settings/${encodeURIComponent(key)}`, { value });
export const getStreamClients = (): Promise<string[]> => fetchJson('/api/settings/stream-clients');
export const clearCaches = (): Promise<void> => postJson('/api/settings/clear-caches', undefined);
// Desktop-only, no browser equivalent (master plan's scope cuts) — permanently unsupported.
export const setAppIcon = (_path: string | null): Promise<void> => unsupported('setAppIcon');
export const appIconPath = (): Promise<string | null> => unsupported('appIconPath');
export const allowFontFile = (_path: string): Promise<void> => unsupported('allowFontFile');

// --- global hotkeys -------------------------------------------------------------------------
export const getGlobalHotkeys = (): Promise<HotkeysConfig> => unsupported('getGlobalHotkeys');
export const globalHotkeysOnWayland = (): Promise<boolean> =>
	unsupported('globalHotkeysOnWayland');
export const setGlobalHotkeys = (_config: HotkeysConfig): Promise<HotkeyRegisterResult> =>
	unsupported('setGlobalHotkeys');
export const resetGlobalHotkeys = (): Promise<HotkeyRegisterResult> =>
	unsupported('resetGlobalHotkeys');

export const releaseNotes = (): Promise<ReleaseNote[]> => unsupported('releaseNotes');
export const canSelfUpdate = (): Promise<boolean> => unsupported('canSelfUpdate');
export const checkBetaUpdate = (): Promise<
	ConstructorParameters<typeof import('@tauri-apps/plugin-updater').Update>[0] | null
> => unsupported('checkBetaUpdate');
export const openExternal = (_url: string): Promise<void> => unsupported('openExternal');

export const diagnostics = (): Promise<string> => fetchText('/api/diagnostics');
export const diagnosticsSummary = (): Promise<string> => fetchText('/api/diagnostics/summary');
/** `path` is a desktop file-save destination, unused here: `/api/diagnostics/save`'s own
 *  `Content-Disposition: attachment` header makes the browser's native download flow handle it —
 *  opening the URL is enough, there's no blob to read back into JS first. */
export const saveDiagnostics = async (_path: string): Promise<void> => {
	window.open(`${SERVER_URL}/api/diagnostics/save`, '_blank');
};

// --- auth (context/15) ---------------------------------------------------------------------
// No webview to host the in-app Google login here — cookie-paste is the only web sign-in method
// (design.md). `signInWithCookie` has no Tauri-side equivalent command (desktop only ever reaches
// `AppState::sign_in` through the hidden-webview flow), so `api.tauri.ts` stubs it the same way
// this file stubs desktop-only commands.
export const getAccount = (): Promise<Account> => fetchJson('/api/auth/account');
export const getAccountIdentities = (): Promise<AccountIdentity[]> =>
	fetchJson('/api/auth/identities');
export const switchAccount = (selectionKey: string): Promise<Account> =>
	postJson('/api/auth/switch', { selectionKey });
export const signOut = (): Promise<void> => postJson('/api/auth/signout', undefined);
export const signInWithCookie = (cookie: string): Promise<Account> =>
	postJson('/api/auth/cookie', { cookie });
export const loginWebview = (_addAccount = false): Promise<void> =>
	unsupported('loginWebview (use signInWithCookie instead)');
export const getGoogleAccounts = (): Promise<SavedAccount[]> =>
	fetchJson('/api/auth/google-accounts');
export const switchGoogleAccount = (id: string): Promise<Account> =>
	postJson('/api/auth/google-accounts/switch', { id });
export const removeGoogleAccount = (id: string): Promise<void> =>
	postJson('/api/auth/google-accounts/remove', { id });

// --- mini player (Rust mini.rs) ---------------------------------------------------------------
export const openMini = (): Promise<void> => unsupported('openMini');
export const closeMini = (): Promise<void> => unsupported('closeMini');
export const setMiniCompact = (_compact: boolean): Promise<void> => unsupported('setMiniCompact');

// --- browse / library (context/08) ---------------------------------------------------------
export const getHome = (params?: string): Promise<HomePage> =>
	fetchJson(`/api/home${params ? `?params=${encodeURIComponent(params)}` : ''}`);
export const getHomeMore = (token: string): Promise<HomePage> =>
	fetchJson(`/api/home/more?token=${encodeURIComponent(token)}`);
export const getLibrary = (): Promise<BrowseItem[]> => fetchJson('/api/library');
// Desktop-local SQLite playlists (local.rs) — no browser equivalent, stays unsupported (design.md).
export const getLocalPlaylists = (): Promise<BrowseItem[]> => unsupported('getLocalPlaylists');
export const getLibraryAlbums = (): Promise<BrowseItem[]> => fetchJson('/api/library/albums');
export const getLibraryArtists = (): Promise<BrowseItem[]> => fetchJson('/api/library/artists');
export const getUploadAlbums = (): Promise<BrowseItem[]> => fetchJson('/api/library/uploads');
export const getHistory = (): Promise<HistoryGroup[]> => fetchJson('/api/history');
export const getPlaylist = (
	id: string,
	sort?: ServerSort,
	desc?: boolean
): Promise<PlaylistPage> => {
	const qs = new URLSearchParams();
	if (sort) qs.set('sort', sort);
	if (desc !== undefined) qs.set('desc', String(desc));
	const suffix = qs.size > 0 ? `?${qs}` : '';
	return fetchJson(`/api/playlist/${encodeURIComponent(id)}${suffix}`);
};
export const setPlaylistSort = (playlistId: string, sort: ServerSort): Promise<void> =>
	putJson(`/api/playlists/${encodeURIComponent(playlistId)}/sort`, { sort });
export const getPlaylistMore = (token: string): Promise<PlaylistContinuation> =>
	fetchJson(`/api/playlist/more?token=${encodeURIComponent(token)}`);
export const playlistIndex = (): Promise<Record<string, string[]>> =>
	unsupported('playlistIndex');
export const syncPlaylistIndex = (): Promise<Record<string, string[]>> =>
	unsupported('syncPlaylistIndex');
export const getPlayCounts = (): Promise<Record<string, number>> => fetchJson('/api/play-counts');
export const playPlaylist = (
	items: SongItem[],
	start: number | null,
	sourceId?: string,
	sourceName?: string,
	shuffle?: boolean,
	_continuation?: string
): Promise<void> => wp.playPlaylist(items, start, sourceId, sourceName, shuffle);
/** Radio resolution (YouTube's own continuation-generated endless queue) needs a server-side route
 *  this change didn't build — the master plan's streaming scope was the resolve/stream/proxy
 *  pipeline, not browse-side radio generation. Stays unsupported; a real gap, not silently
 *  dropped — flagged in tasks.md for a later change. */
export const startRadio = (
	_kind: 'song' | 'artist' | 'album' | 'playlist',
	_id: string,
	_name?: string
): Promise<void> => unsupported('startRadio');
export const getAlbum = (id: string): Promise<AlbumPage> =>
	fetchJson(`/api/album/${encodeURIComponent(id)}`);
export const getArtist = (id: string): Promise<ArtistPage> =>
	fetchJson(`/api/artist/${encodeURIComponent(id)}`);
export const getMoods = (): Promise<MoodSection[]> => fetchJson('/api/moods');
export const getMoodArt = (params: string[]): Promise<Record<string, string>> =>
	postJson('/api/moods/art', params);
export const getBrowseGrid = (id: string, params?: string): Promise<BrowseItem[]> =>
	fetchJson(
		`/api/browse/${encodeURIComponent(id)}${params ? `?params=${encodeURIComponent(params)}` : ''}`
	);

// --- local music (local.rs) ------------------------------------------------------------------
export const getLocalLibrary = (): Promise<LocalLibrary> => unsupported('getLocalLibrary');
export const addLocalFolder = (_path: string): Promise<LocalLibrary> =>
	unsupported('addLocalFolder');
export const removeLocalFolder = (_path: string): Promise<LocalLibrary> =>
	unsupported('removeLocalFolder');

// --- blocked artists (blocked.rs, plan 046) — anonymous, no account concept ------------------
export const getBlockedArtists = (): Promise<BlockedArtist[]> => fetchJson('/api/blocked-artists');
export const blockArtist = (id: string | undefined, name: string): Promise<BlockedArtist[]> =>
	postJson('/api/blocked-artists', { id, name });
export const unblockArtist = (key: string): Promise<BlockedArtist[]> =>
	deleteJson(`/api/blocked-artists/${encodeURIComponent(key)}`);

// --- write actions (context/01 ✎) ----------------------------------------------------------
export const rate = (videoId: string, rating: Rating): Promise<void> =>
	putJson(`/api/songs/${encodeURIComponent(videoId)}/rating`, { rating });
export const addToPlaylist = (
	playlistId: string,
	videoId: string,
	allowDuplicates = false
): Promise<boolean> =>
	postJson(`/api/playlists/${encodeURIComponent(playlistId)}/tracks`, {
		videoId,
		allowDuplicates
	});
export const removeFromPlaylist = (
	playlistId: string,
	videoId: string,
	setVideoId: string
): Promise<void> =>
	deleteJson(`/api/playlists/${encodeURIComponent(playlistId)}/tracks`, {
		videoId,
		setVideoId
	});
export const removeManyFromPlaylist = (
	playlistId: string,
	tracks: [string, string][]
): Promise<void> =>
	deleteJson(`/api/playlists/${encodeURIComponent(playlistId)}/tracks`, { tracks });
export const createPlaylist = (title: string, local = false): Promise<string> =>
	local
		? unsupported('createPlaylist(local: true) — desktop-local playlists have no browser equivalent')
		: postJson('/api/playlists', { title });
export const addToLocalPlaylist = (
	_playlistId: string,
	_items: SongItem[]
): Promise<boolean[]> => unsupported('addToLocalPlaylist');
export const editPlaylistDetails = (
	playlistId: string,
	changes: { name?: string; description?: string; public?: boolean }
): Promise<void> => patchJson(`/api/playlists/${encodeURIComponent(playlistId)}`, changes);
/** `path` means a native filesystem path on desktop; here, a browser object URL
 *  (`URL.createObjectURL(file)`) a caller creates from a picked `File` — same string-typed
 *  signature both platforms share, different meaning per platform. `null` clears the cover. */
export const setPlaylistCover = async (
	playlistId: string,
	path: string | null
): Promise<{ cover?: string; thumbnail?: string }> => {
	const url = `/api/playlists/${encodeURIComponent(playlistId)}/cover`;
	if (path === null) {
		const thumbnail = await putBytes<string | null>(url, new Uint8Array(0));
		return thumbnail ? { thumbnail } : {};
	}
	const blob = await fetch(path).then((r) => r.blob());
	await putBytes(url, blob);
	return {};
};
export const deletePlaylist = (playlistId: string): Promise<void> =>
	deleteJson(`/api/playlists/${encodeURIComponent(playlistId)}`);
export const subscribe = (channelId: string, subscribed: boolean): Promise<void> =>
	putJson(`/api/artists/${encodeURIComponent(channelId)}/subscribed`, { subscribed });
export const setSongSaved = (token: string): Promise<void> =>
	putJson('/api/songs/saved', { token });
export const setAlbumSaved = (playlistId: string, saved: boolean): Promise<void> =>
	putJson(`/api/albums/${encodeURIComponent(playlistId)}/saved`, { saved });

// --- events (context/11). None of these ever fire in the web build: there is no Tauri event
// stream to subscribe to, so each resolves with a no-op unlisten rather than rejecting (unlike the
// commands above, nothing here is a call a caller awaits the result *of* — it's a subscription
// callers mount and forget, so leaving it permanently silent is the correct inert behavior). ------
export const onNowPlaying = (_cb: (n: NowPlaying) => void): Promise<UnlistenFn> => noSubscription();
export const onRating = (_cb: (videoId: string, rating: Rating) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onVideoReady = (_cb: (videoId: string) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onQueueChanged = (_cb: (q: QueueState) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onQueueIndex = (_cb: (q: QueueIndex) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onQueueAppended = (_cb: (q: QueueAppended) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onUiVisible = (_cb: (v: boolean) => void): Promise<UnlistenFn> => noSubscription();
export const takeLaunchArgs = (): Promise<string[]> => unsupported('takeLaunchArgs');
export const onOpenLink = (_cb: (args: string[]) => void): Promise<UnlistenFn> => noSubscription();
export const onPosition = (_cb: (p: number) => void): Promise<UnlistenFn> => noSubscription();
export const onDuration = (_cb: (d: number) => void): Promise<UnlistenFn> => noSubscription();
export const onVolume = (_cb: (v: number) => void): Promise<UnlistenFn> => noSubscription();
export const onPlaybackState = (
	_cb: (s: 'playing' | 'paused') => void
): Promise<UnlistenFn> => noSubscription();
export const onPlaybackError = (_cb: (msg: string) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onPlaybackNotice = (_cb: (msg: string) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onCoverError = (_cb: (msg: string) => void): Promise<UnlistenFn> => noSubscription();
export const onAuthChanged = (cb: (a: Account) => void): Promise<UnlistenFn> =>
	Promise.resolve(subscribeWs('auth-changed', (data) => cb(data as Account)));
export const onAccountSelectionRequired = (cb: () => void): Promise<UnlistenFn> =>
	Promise.resolve(subscribeWs('account-selection-required', () => cb()));
export const onLocalChanged = (_cb: (removed: string[]) => void): Promise<UnlistenFn> =>
	noSubscription();
export const onLoginError = (_cb: (msg: string) => void): Promise<UnlistenFn> => noSubscription();
export const onLoginDone = (_cb: () => void): Promise<UnlistenFn> => noSubscription();

// --- lyrics (anonymous — no session needed, matching the original Tauri commands) -----------
function lyricsQuery(args: LyricsTrack): URLSearchParams {
	const qs = new URLSearchParams({ video_id: args.videoId, title: args.title, artists: args.artists });
	if (args.album) qs.set('album', args.album);
	if (args.duration !== undefined) qs.set('duration', String(args.duration));
	return qs;
}
export const getLyrics = (args: LyricsTrack & { source?: string }): Promise<Lyrics | null> =>
	fetchJson(`/api/lyrics?${lyricsQuery(args)}`);
export const chooseLyricsSource = (
	args: LyricsTrack & { source: string | null }
): Promise<Lyrics | null> => {
	const qs = lyricsQuery(args);
	if (args.source) qs.set('source', args.source);
	return fetchJson(`/api/lyrics/choose-source?${qs}`);
};
export const setLyricsOffset = (videoId: string, offsetMs: number): Promise<void> =>
	putJson(`/api/lyrics/${encodeURIComponent(videoId)}/offset`, { offsetMs });
export const lyricsProviders = (): Promise<LyricsProvider[]> => fetchJson('/api/lyrics/providers');

// --- Window ------------------------------------------------------------------------------------
export const theaterFullscreen = (_on: boolean): Promise<void> => unsupported('theaterFullscreen');

// --- Last.fm scrobbling ---------------------------------------------------------------------
export const lastfmStatus = (): Promise<LastfmState> => fetchJson('/api/lastfm/status');
/** The server has no browser of its own to open the authorize page from (unlike the desktop
 *  command) — it hands back the URL instead, and this opens it here, in the caller's own browser,
 *  keeping `Titlebar.svelte`'s shared call site (`await api.lastfmConnect()`) unchanged. */
export const lastfmConnect = async (): Promise<void> => {
	const { authorizeUrl } = await postJson<{ authorizeUrl: string }>('/api/lastfm/connect', undefined);
	window.open(authorizeUrl, '_blank', 'noopener');
};
export const lastfmDisconnect = (): Promise<void> => postJson('/api/lastfm/disconnect', undefined);
export const onLastfmState = (cb: (s: LastfmState) => void): Promise<UnlistenFn> =>
	Promise.resolve(subscribeWs('lastfm-state', (data) => cb(data as LastfmState)));

// --- Listen Together (context/19) -----------------------------------------------------------
// A direct WebSocket to the separate `crates/sync-server` relay, not this server — see
// lt-client.ts's header comment for why there's no Rust work behind any of these.
export const ltGetState = (): Promise<LtState> => Promise.resolve(lt.getState());
export const ltSetServerUrl = (url: string): Promise<void> => {
	lt.setServerUrl(url);
	return Promise.resolve();
};
export const ltCreateRoom = (username: string): Promise<void> => {
	lt.createRoom(username);
	return Promise.resolve();
};
export const ltJoinRoom = (code: string, username: string): Promise<void> => {
	lt.joinRoom(code, username);
	return Promise.resolve();
};
export const ltLeave = (): Promise<void> => {
	lt.leave();
	return Promise.resolve();
};
export const ltApproveJoin = (userId: string): Promise<void> => {
	lt.approveJoin(userId);
	return Promise.resolve();
};
export const ltRejectJoin = (userId: string): Promise<void> => {
	lt.rejectJoin(userId);
	return Promise.resolve();
};
export const ltKick = (userId: string): Promise<void> => {
	lt.kick(userId);
	return Promise.resolve();
};
export const ltTransferHost = (userId: string): Promise<void> => {
	lt.transferHost(userId);
	return Promise.resolve();
};
export const ltApproveSuggestion = (id: string): Promise<void> => {
	lt.approveSuggestion(id);
	return Promise.resolve();
};
export const ltRejectSuggestion = (id: string): Promise<void> => {
	lt.rejectSuggestion(id);
	return Promise.resolve();
};
export const ltRequestSync = (): Promise<void> => {
	lt.requestSync();
	return Promise.resolve();
};

export const onLtState = (cb: (s: LtState) => void): Promise<UnlistenFn> =>
	Promise.resolve(lt.onState(cb));
export const onLtNotice = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	Promise.resolve(lt.onNotice(cb));
