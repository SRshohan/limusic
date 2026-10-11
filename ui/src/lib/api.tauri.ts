// The desktop implementation of the UI's command surface: every call here is a Tauri IPC round
// trip (`invoke`) or event subscription (`listen`) into the Rust backend. context/11 UI contract —
// commands in, events out. The UI never touches YouTube directly; everything here is a Tauri
// command or event payload. Never imported directly outside `api.ts` — see platform.ts/api.ts for
// why this and `api.web.ts` are picked between at module load instead of branching per call.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { t } from './i18n.svelte';
import {
	ON_REPEAT_ID,
	isLocalPlaylist,
	type Account,
	type AccountIdentity,
	type AlbumPage,
	type ArtistPage,
	type BlockedArtist,
	type BrowseItem,
	type HistoryGroup,
	type HomePage,
	type HotkeyRegisterResult,
	type HotkeysConfig,
	type LastfmState,
	type LocalLibrary,
	type LtState,
	type Lyrics,
	type LyricsProvider,
	type LyricsTrack,
	type NowPlaying,
	type PlaybackSnapshot,
	type PlaylistContinuation,
	type PlaylistPage,
	type QueueAppended,
	type QueueIndex,
	type QueueState,
	type Rating,
	type ReleaseNote,
	type RepeatMode,
	type SavedAccount,
	type SearchResults,
	type SearchSuggestions,
	type ServerSort,
	type SongItem,
	type MoodSection
} from './api.types';

export * from './api.types';

// --- commands (context/11) -----------------------------------------------------------------
// `recordHistory` is true only for a query the user submitted: a signed-in search is written to the
// account's YouTube search history, so a typeahead preview must stay anonymous (#203).
export const search = (query: string, recordHistory = false) =>
	invoke<SongItem[]>('search', { query, recordHistory });
/** Video uploads only: covers, live sets and remixes with no official release. Empty when the
 *  "hide music videos" setting is on. */
export const searchVideos = (query: string) => invoke<SongItem[]>('search_videos', { query });
/** Unfiltered search → categorized sections. */
export const searchAll = (query: string, recordHistory = false) =>
	invoke<SearchResults>('search_all', { query, recordHistory });
/** The typeahead. Signed in, yet never written to search history: it's the request YTM's own
 *  search box sends on every keystroke. */
export const searchSuggestions = (query: string) =>
	invoke<SearchSuggestions>('search_suggestions', { query });
/** Filtered "Show more" card search for one category (albums / artists / playlists). */
export const searchCards = (query: string, category: 'albums' | 'artists' | 'playlists') =>
	invoke<BrowseItem[]>('search_cards', { query, category });
export const play = (item: SongItem) => invoke<void>('play', { item });
export const playIndex = (index: number) => invoke<void>('play_index', { index });
/** Remove an upcoming track from the queue (host/local only — guests are add-only). */
export const removeFromQueue = (index: number) => invoke<void>('remove_from_queue', { index });
/** Drag-to-reorder: move the upcoming queue item at `from` to index `to` (both past the playing
 * track — the history and the playing row don't move). */
export const moveInQueue = (from: number, to: number) =>
	invoke<void>('move_in_queue', { from, to });
/**
 * "Play next": insert tracks at the front of the "Next in queue" block, behind any earlier
 * "Play next" adds. `from` is the album/playlist they came from — it heads the block in the panel.
 */
export const playNext = (items: SongItem[], from?: string) =>
	invoke<void>('play_next', { items, from });
/**
 * "Add to queue": the tracks go at the *back* of the same block — after everything already queued
 * by hand, ahead of the playing context and anything the app generated behind it.
 * `continuation` is the source page's next-page token — the backend walks the rest of a long
 * playlist into the queue in the background.
 */
export const addToQueue = (items: SongItem[], from?: string, continuation?: string) =>
	invoke<void>('add_to_queue', { items, from, continuation });
/** Clear every upcoming manually-queued track (the "Next in queue" section). */
export const clearQueued = () => invoke<void>('clear_queued');
export const nextTrack = () => invoke<void>('next_track');
export const prevTrack = () => invoke<void>('prev_track');
/** Put back the queue a click replaced, at the track and position it was left at. Previous does
 *  this too, but only from the top of a track. */
export const backToPrevious = () => invoke<void>('back_to_previous');
export const toggleShuffle = () => invoke<void>('toggle_shuffle');
export const setRepeat = (mode: RepeatMode) => invoke<void>('set_repeat', { mode });
export const togglePause = () => invoke<void>('toggle_pause');
export const seek = (position: number) => invoke<void>('seek', { position });
export const setVolume = (volume: number) => invoke<void>('set_volume', { volume });
/** Tempo (0.25–2.0) + pitch (−12..=12 semitones). Not persisted: resets on restart. */
export const setPlaybackParams = (speed: number, semitones: number) =>
	invoke<void>('set_playback_params', { speed, semitones });
export const getQueue = () => invoke<QueueState>('get_queue');
/** A `limusicvideo://` URL for the track's music video, or null when there isn't one. `maxHeight`
 *  caps the picture at what the box on screen can actually show. The bytes are proxied through
 *  Rust; the webview never sees a googlevideo URL. */
export const videoStream = (videoId: string, maxHeight: number) =>
	invoke<string | null>('video_stream', { videoId, maxHeight });

/** Drop the backend's memory of this track's video URL, after the element failed to load it. */
export const forgetVideoStream = (videoId: string) =>
	invoke<void>('forget_video_stream', { videoId });

/** Linux: where the page's hole for the music video is (`[x, y, w, h]`, CSS pixels, relative to
 *  the viewport), or null when there is none. mpv draws the picture there, under the webview. Resolves
 *  whether the picture is up; `false` for a rect means it never will be (no GL), so fall back to
 *  the `<video>` element. */
export const nativeVideoRect = (rect: [number, number, number, number] | null) =>
	invoke<boolean>('native_video_rect', { rect });

/** Linux: the newest small frame of mpv's picture other than `after`, for the ambient light, as
 *  `[seq, w, h]` little-endian u32s and then RGBA rows bottom-up. Empty when none came within a
 *  quarter second. Asking is also what keeps Rust grabbing them (nativevideo.rs).
 *  An ArrayBuffer, except once Tauri has fallen back from its custom protocol to postMessage (it
 *  does for the rest of the page's life after any IPC fetch fails): raw bytes then arrive as a
 *  plain array of numbers. */
export const ambientFrame = (after: number) =>
	invoke<ArrayBuffer | number[]>('ambient_frame', { after });

export const getPlayback = () => invoke<PlaybackSnapshot>('get_playback');

// --- settings (context/11) -----------------------------------------------------------------
export const getSettings = () => invoke<Record<string, string>>('get_settings');
export const setSetting = (key: string, value: string) =>
	invoke<void>('set_setting', { key, value });
/** Streamable client keys for the "disabled clients" setting. */
export const getStreamClients = () => invoke<string[]>('get_stream_clients');
/** Wipe both cache tiers (URL cache + mpv on-disk audio cache). */
export const clearCaches = () => invoke<void>('clear_caches');
/** Set the app icon to a PNG the user picked, or restore the bundled one with `null` (#173). */
export const setAppIcon = (path: string | null) => invoke<void>('set_app_icon', { path });
/** Path to the custom app icon, granted to the asset protocol. `null` when the bundled one is in use. */
export const appIconPath = () => invoke<string | null>('app_icon_path');

/** Grant the webview a URL for one font file the user picked, so `@font-face` can load it. */
export const allowFontFile = (path: string) => invoke<void>('allow_font_file', { path });

// --- global hotkeys -------------------------------------------------------------------------
export const getGlobalHotkeys = () => invoke<HotkeysConfig>('get_global_hotkeys');
export const globalHotkeysOnWayland = () => invoke<boolean>('global_hotkeys_on_wayland');
export const setGlobalHotkeys = (config: HotkeysConfig) =>
	invoke<HotkeyRegisterResult>('set_global_hotkeys', { config });
export const resetGlobalHotkeys = () => invoke<HotkeyRegisterResult>('reset_global_hotkeys');

/** Changelog for Settings > About, from the GitHub releases API (cached in Rust per run). */
export const releaseNotes = () => invoke<ReleaseNote[]>('release_notes');
/** False on Linux builds that aren't the AppImage (.rpm, the AUR package): they update through the
 *  package manager, so the UI offers a download link instead of an install button. */
export const canSelfUpdate = () => invoke<boolean>('can_self_update');
/** The updater plugin's `check()` against the beta channel's manifest, as the metadata the
 *  plugin's `Update` class is built from. `null` when this build is what the channel offers. */
export const checkBetaUpdate = () =>
	invoke<ConstructorParameters<typeof import('@tauri-apps/plugin-updater').Update>[0] | null>(
		'check_beta_update'
	);
/** Open an http(s) link in the real browser, never in the webview itself. */
export const openExternal = (url: string) => invoke<void>('open_external', { url });

/** Environment + the redacted tail of `limusic.log`, for pasting into a bug report. */
export const diagnostics = () => invoke<string>('diagnostics');
/** Just the environment block, for prefilling the GitHub bug form. */
export const diagnosticsSummary = () => invoke<string>('diagnostics_summary');
/** The same text, written to a path the user picked in a save dialog. */
export const saveDiagnostics = (path: string) => invoke<void>('save_diagnostics', { path });

// --- auth (context/15) ---------------------------------------------------------------------
export const getAccount = () => invoke<Account>('get_account');
export const getAccountIdentities = () => invoke<AccountIdentity[]>('get_account_identities');
export const switchAccount = (selectionKey: string) =>
	invoke<Account>('switch_account', { selectionKey });
export const signOut = () => invoke<void>('sign_out');
/**
 * Open the in-app Google sign-in webview (context/15 Path A). Result arrives via onAuthChanged.
 * With `addAccount`, Google's AddSession screen is used so a second account can be added even
 * while the webview already holds a Google session.
 */
export const loginWebview = (addAccount = false) => invoke<void>('login_webview', { addAccount });
/** Saved Google accounts for the account menu (display fields only). */
export const getGoogleAccounts = () => invoke<SavedAccount[]>('get_google_accounts');
/** Activate a saved account without a Google re-login. Fails if its stored session expired. */
export const switchGoogleAccount = (id: string) =>
	invoke<Account>('switch_google_account', { id });
/** Delete a saved account; removing the active one signs out. */
export const removeGoogleAccount = (id: string) => invoke<void>('remove_google_account', { id });

// --- mini player (Rust mini.rs) ---------------------------------------------------------------
/** Hide the app to the tray and open the floating widget (a second window running this same SPA). */
export const openMini = () => invoke<void>('open_mini');
/** Close the widget and bring the app back. */
export const closeMini = () => invoke<void>('close_mini');
/** Shrink the widget to its compact size, or back (#301). Remembered for the next open. */
export const setMiniCompact = (compact: boolean) => invoke<void>('set_mini_compact', { compact });

// --- browse / library (context/08) ---------------------------------------------------------
/** `params` is a `HomeChip.params` token — omit for the unfiltered feed. */
export const getHome = (params?: string) => invoke<HomePage>('get_home', { params });
export const getHomeMore = (token: string) => invoke<HomePage>('get_home_more', { token });
/**
 * On Repeat is the app's own playlist (Rust builds it from this machine's play counts), so its
 * title and subtitle are our English rather than YouTube's, and Rust cannot translate them: the
 * UI language lives in the webview's localStorage and never reaches it. Relabelled here, on the
 * way in, because every surface that draws the tile reads it from one of these two calls.
 */
const relabelOnRepeat = (item: BrowseItem): BrowseItem =>
	item.id !== ON_REPEAT_ID
		? item
		: {
				...item,
				title: t('library.on_repeat'),
				// The count is Rust's leading number ("20 songs"); left alone if it ever isn't.
				subtitle: Number.isNaN(parseInt(item.subtitle ?? '', 10))
					? item.subtitle
					: t('library.songs_count', { count: parseInt(item.subtitle!, 10) })
			};

/** Same reason as On Repeat: Rust's "12 songs" on a playlist kept on this machine. */
const relabelLocal = (item: BrowseItem): BrowseItem => {
	if (!isLocalPlaylist(item.id)) return relabelOnRepeat(item);
	const count = parseInt(item.subtitle ?? '', 10);
	if (Number.isNaN(count)) return item;
	return {
		...item,
		subtitle:
			count === 1
				? t('library.local_playlist_subtitle_one')
				: t('library.local_playlist_subtitle', { count })
	};
};

/** Every playlist in the library: On Repeat, the ones on this machine, then the account's. */
export const getLibrary = () =>
	invoke<BrowseItem[]>('get_library').then((items) => items.map(relabelLocal));
/** Just the playlists on this machine. SQLite only, so it answers offline and signed out. */
export const getLocalPlaylists = () =>
	invoke<BrowseItem[]>('local_playlists').then((items) => items.map(relabelLocal));
export const getLibraryAlbums = () => invoke<BrowseItem[]>('get_library_albums');
export const getLibraryArtists = () => invoke<BrowseItem[]>('get_library_artists');
export const getUploadAlbums = () => invoke<BrowseItem[]>('get_upload_albums');
/**
 * The account's YouTube Music play history, in YouTube's own day buckets (Today, Yesterday, …).
 * Empty when signed out.
 */
export const getHistory = () => invoke<HistoryGroup[]>('get_history');
/**
 * `sort` asks YouTube to order the tracks; omit it to get whatever order the account already has
 * the list in, which is the one a fresh visit wants (it is what YouTube Music would show).
 */
export const getPlaylist = (id: string, sort?: ServerSort, desc?: boolean) =>
	invoke<PlaylistPage>('get_playlist', { id, sort, desc }).then((page) =>
		id !== ON_REPEAT_ID
			? page
			: {
					...page,
					title: t('library.on_repeat'),
					subtitle: t('library.on_repeat_subtitle', { count: page.items.length })
				}
	);
/**
 * Store a sort order on a playlist, so YouTube Music and every other client show it the same way.
 * Only for a list whose `sortMenu.editable` is true.
 */
export const setPlaylistSort = (playlistId: string, sort: ServerSort) =>
	invoke<void>('set_playlist_sort', { playlistId, sort });
export const getPlaylistMore = (token: string) =>
	invoke<PlaylistContinuation>('get_playlist_more', { token });
/**
 * videoId → the ids of the playlists you own that hold it. Read straight from local SQLite, so it
 * answers instantly and is empty until `syncPlaylistIndex` has filled it in at least once.
 */
export const playlistIndex = () => invoke<Record<string, string[]>>('playlist_index');
/**
 * Re-walk your own playlists and answer with the rebuilt map. Skips the crawl while the stored one
 * is still inside its window, so calling this on every launch is cheap.
 */
export const syncPlaylistIndex = () => invoke<Record<string, string[]>>('sync_playlist_index');
/**
 * videoId → times played, from the local listening history. Same trailing window On Repeat uses
 * (a month): the history table is pruned to it, so there is no older data. A videoId that isn't in
 * the map has not been played inside the window.
 */
export const getPlayCounts = () => invoke<Record<string, number>>('play_counts');
/**
 * `start`: the clicked track index, or `null` for "just play it" (random opener under shuffle).
 * `sourceId`: the page's playlist/album playlist id — makes autoplay continue with that
 * context's radio (omit to fall back to song radio seeded from the queue's last track).
 * `sourceName`: the page title, for the queue panel's "Next from" header.
 * `shuffle`: turn shuffle on for this queue — pass items in their real order, Rust shuffles.
 */
export const playPlaylist = (
	items: SongItem[],
	start: number | null,
	sourceId?: string,
	sourceName?: string,
	shuffle?: boolean,
	continuation?: string
) => invoke<void>('play_playlist', { items, start, sourceId, sourceName, shuffle, continuation });
/**
 * Start a radio: an endless YouTube-generated queue seeded on this item. `id` is the videoId
 * (song) or browseId/playlistId (everything else) — Rust resolves it to a radio playlist, so the
 * UI never builds one. `name` titles the queue ("<name> Radio").
 *
 * A song radio on the track that's already playing splices in behind it (no re-buffer); every
 * other case replaces the queue. Rejects when YouTube has no radio for the item.
 */
export const startRadio = (kind: 'song' | 'artist' | 'album' | 'playlist', id: string, name?: string) =>
	invoke<void>('start_radio', { kind, id, name });
export const getAlbum = (id: string) => invoke<AlbumPage>('get_album', { id });
export const getArtist = (id: string) => invoke<ArtistPage>('get_artist', { id });
export const getMoods = () => invoke<MoodSection[]>('get_moods');
/** A cover per tile (the first playlist in its category), keyed by the tile's `params`. */
export const getMoodArt = (params: string[]) =>
	invoke<Record<string, string>>('get_mood_art', { params });
export const getBrowseGrid = (id: string, params?: string) =>
	invoke<BrowseItem[]>('get_browse_grid', { id, params });

// --- local music (local.rs) ------------------------------------------------------------------
/** Rescan the watched folders. Cheap when nothing changed (one stat per file). */
export const getLocalLibrary = () => invoke<LocalLibrary>('get_local_library');
export const addLocalFolder = (path: string) => invoke<LocalLibrary>('add_local_folder', { path });
export const removeLocalFolder = (path: string) =>
	invoke<LocalLibrary>('remove_local_folder', { path });

// --- blocked artists (blocked.rs, plan 046) ---------------------------------------------------
export const getBlockedArtists = () => invoke<BlockedArtist[]>('get_blocked_artists');
/** Blocks the artist and returns the new list. Rust also drops them out of the live queue. */
export const blockArtist = (id: string | undefined, name: string) =>
	invoke<BlockedArtist[]>('block_artist', { id, name });
/** `key` is the entry's channel id when it has one, else its name. */
export const unblockArtist = (key: string) => invoke<BlockedArtist[]>('unblock_artist', { key });

// --- write actions (context/01 ✎) ----------------------------------------------------------
/** Like, dislike, or clear the rating. YouTube's three states are mutually exclusive, so a dislike
 *  un-likes in the same call. */
export const rate = (videoId: string, rating: Rating) => invoke<void>('rate', { videoId, rating });
/** `false` = the playlist already had this track, so YouTube added nothing. */
export const addToPlaylist = (playlistId: string, videoId: string, allowDuplicates = false) =>
	invoke<boolean>('add_to_playlist', { playlistId, videoId, allowDuplicates });
export const removeFromPlaylist = (playlistId: string, videoId: string, setVideoId: string) =>
	invoke<void>('remove_from_playlist', { playlistId, videoId, setVideoId });

/** Bulk removal: one request, all or nothing. `tracks` is [videoId, setVideoId] per row. */
export const removeManyFromPlaylist = (playlistId: string, tracks: [string, string][]) =>
	invoke<void>('remove_many_from_playlist', { playlistId, tracks });
/** `local` keeps it on this machine instead of the account, the only kind there is signed out.
 *  Answers the new id: a `LOCALPLAYLIST:` browseId for a local one, YouTube's playlist id else. */
export const createPlaylist = (title: string, local = false) =>
	invoke<string>('create_playlist', { title, local });
/** Add whole songs to a playlist on this machine, in one write. Answers per song whether it went
 *  in: `false` means the playlist already had it. Local files are fine here. */
export const addToLocalPlaylist = (playlistId: string, items: SongItem[]) =>
	invoke<boolean[]>('add_to_local_playlist', { playlistId, items });
/** Name / description / visibility, from the "Edit playlist" dialog. Leave a field out and
 *  YouTube is never told about it, so an untouched one can't be overwritten. */
export const editPlaylistDetails = (
	playlistId: string,
	changes: { name?: string; description?: string; public?: boolean }
) => invoke<void>('edit_playlist_details', { playlistId, ...changes });
/** Custom playlist artwork. `path` is a file the user picked; `null` drops it. Answers where the
 *  local copy went, and on a removal the thumbnail YouTube rebuilt from the tracks (that one is
 *  worth waiting for: YouTube's own thumbnail is the cover being removed until it lands). */
export const setPlaylistCover = (playlistId: string, path: string | null) =>
	invoke<{ cover?: string; thumbnail?: string }>('set_playlist_cover', { playlistId, path });
export const deletePlaylist = (playlistId: string) =>
	invoke<void>('delete_playlist', { playlistId });
export const subscribe = (channelId: string, subscribed: boolean) =>
	invoke<void>('subscribe', { channelId, subscribed });
/** Add a song to Library ▸ Songs, or take it out. `token` is `SongItem.library.add_token` /
 *  `.remove_token`; YouTube mints them per row, so they come from the list the song was shown in. */
export const setSongSaved = (token: string) => invoke<void>('set_song_saved', { token });
/** Save an album to the library (or remove it). `playlistId` is `AlbumPage.playlistId`. */
export const setAlbumSaved = (playlistId: string, saved: boolean) =>
	invoke<void>('set_album_saved', { playlistId, saved });

// --- events (context/11). Each returns an unlisten fn; call it on component teardown. --------
export const onNowPlaying = (cb: (n: NowPlaying) => void): Promise<UnlistenFn> =>
	listen<NowPlaying>('now-playing', (e) => cb(e.payload));
/**
 * The backend asked YouTube what a track's rating really is and got a different answer than the
 * row we were handed (issue #93). Fires only on a change, at most once per track start.
 */
export const onRating = (cb: (videoId: string, rating: Rating) => void): Promise<UnlistenFn> =>
	listen<{ videoId: string; rating: Rating }>('rating', (e) =>
		cb(e.payload.videoId, e.payload.rating)
	);
/** Linux: mpv has this track's music video (or will as soon as the track starts). */
export const onVideoReady = (cb: (videoId: string) => void): Promise<UnlistenFn> =>
	listen<string>('video-ready', (e) => cb(e.payload));
export const onQueueChanged = (cb: (q: QueueState) => void): Promise<UnlistenFn> =>
	listen<QueueState>('queue-changed', (e) => cb(e.payload));

export const onQueueIndex = (cb: (q: QueueIndex) => void): Promise<UnlistenFn> =>
	listen<QueueIndex>('queue-index', (e) => cb(e.payload));

export const onQueueAppended = (cb: (q: QueueAppended) => void): Promise<UnlistenFn> =>
	listen<QueueAppended>('queue-appended', (e) => cb(e.payload));
/** Main window shown/hidden (close-to-tray, the mini player). WebKitGTK never tells the page. */
export const onUiVisible = (cb: (v: boolean) => void): Promise<UnlistenFn> =>
	listen<boolean>('ui-visible', (e) => cb(e.payload));
/** `limusic-app <link>` (#348): the arguments a cold launch was given, handed over once... */
export const takeLaunchArgs = () => invoke<string[]>('take_launch_args');
/** ...and those of a second launch while this one runs. */
export const onOpenLink = (cb: (args: string[]) => void): Promise<UnlistenFn> =>
	listen<string[]>('open-link', (e) => cb(e.payload));
export const onPosition = (cb: (p: number) => void): Promise<UnlistenFn> =>
	listen<{ position: number }>('position', (e) => cb(e.payload.position));
export const onDuration = (cb: (d: number) => void): Promise<UnlistenFn> =>
	listen<{ duration: number }>('duration', (e) => cb(e.payload.duration));
/** Echo of every `set_volume`, so a second window's slider can't drift from what you hear. */
export const onVolume = (cb: (v: number) => void): Promise<UnlistenFn> =>
	listen<number>('volume', (e) => cb(e.payload));
export const onPlaybackState = (cb: (s: 'playing' | 'paused') => void): Promise<UnlistenFn> =>
	listen<'playing' | 'paused'>('playback-state', (e) => cb(e.payload));
export const onPlaybackError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<{ message: string }>('playback-error', (e) => cb(e.payload.message));
export const onPlaybackNotice = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<{ message: string }>('playback-notice', (e) => cb(e.payload.message));
/** Custom playlist artwork applied here but refused by YouTube Music (it syncs in the background,
 *  so the failure lands long after the picker closed). */
export const onCoverError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<{ message: string }>('cover-error', (e) => cb(e.payload.message));
export const onAuthChanged = (cb: (a: Account) => void): Promise<UnlistenFn> =>
	listen<Account>('auth-changed', (e) => cb(e.payload));
export const onAccountSelectionRequired = (cb: () => void): Promise<UnlistenFn> =>
	listen('account-selection-required', () => cb());
/**
 * Local music disappeared from disk. Fired when a play attempt finds nothing there, carrying the
 * song (and album, if that emptied it) so every view holding those ids can drop them at once.
 */
export const onLocalChanged = (cb: (removed: string[]) => void): Promise<UnlistenFn> =>
	listen<{ removed: string[] }>('local-changed', (e) => cb(e.payload.removed));
export const onLoginError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<string>('login-error', (e) => cb(e.payload));
export const onLoginDone = (cb: () => void): Promise<UnlistenFn> => listen('login-done', () => cb());

// --- lyrics ---------------------------------------------------------------------------------
/** Cached on the Rust side, down the user's provider order. `null` = none found. `source` asks that
 *  one provider alone and caches nothing (the source picker's preview); it rejects when the
 *  provider couldn't be reached, which is not the same as it having no lyrics. */
export const getLyrics = (args: LyricsTrack & { source?: string }) =>
	invoke<Lyrics | null>('get_lyrics', args);
/** Keep `source`'s lyrics for this song, or (`null`) hand it back to the provider order. */
export const chooseLyricsSource = (args: LyricsTrack & { source: string | null }) =>
	invoke<Lyrics | null>('choose_lyrics_source', args);
export const setLyricsOffset = (videoId: string, offsetMs: number) =>
	invoke<void>('set_lyrics_offset', { videoId, offsetMs });
/** Every provider in the user's order (setting `lyrics_providers`: ids, `-id` switched off). */
export const lyricsProviders = () => invoke<LyricsProvider[]>('lyrics_providers');

// --- Window ------------------------------------------------------------------------------------
/** Theater mode's fullscreen. Not `getCurrentWindow().setFullscreen` (#139): Windows needs the
 *  maximized state undone first and the frame recalculated after, in that order, on the main
 *  thread. Rust also puts the maximized state back when theater closes. */
export const theaterFullscreen = (on: boolean) => invoke<void>('theater_fullscreen', { on });

// --- Last.fm scrobbling ---------------------------------------------------------------------
export const lastfmStatus = () => invoke<LastfmState>('lastfm_status');
/** Opens the browser auth flow; the outcome arrives via onLastfmState, not this promise. */
export const lastfmConnect = () => invoke<void>('lastfm_connect');
/** Also cancels an in-flight connect (the auth poll checks and bails). */
export const lastfmDisconnect = () => invoke<void>('lastfm_disconnect');
export const onLastfmState = (cb: (s: LastfmState) => void): Promise<UnlistenFn> =>
	listen<LastfmState>('lastfm-state', (e) => cb(e.payload));

// --- Listen Together (context/19) -----------------------------------------------------------
export const ltGetState = () => invoke<LtState>('lt_get_state');
export const ltSetServerUrl = (url: string) => invoke<void>('lt_set_server_url', { url });
export const ltCreateRoom = (username: string) => invoke<void>('lt_create_room', { username });
export const ltJoinRoom = (code: string, username: string) =>
	invoke<void>('lt_join_room', { code, username });
export const ltLeave = () => invoke<void>('lt_leave');
export const ltApproveJoin = (userId: string) => invoke<void>('lt_approve_join', { userId });
export const ltRejectJoin = (userId: string) => invoke<void>('lt_reject_join', { userId });
export const ltKick = (userId: string) => invoke<void>('lt_kick', { userId });
export const ltTransferHost = (userId: string) => invoke<void>('lt_transfer_host', { userId });
export const ltApproveSuggestion = (id: string) => invoke<void>('lt_approve_suggestion', { id });
export const ltRejectSuggestion = (id: string) => invoke<void>('lt_reject_suggestion', { id });
export const ltRequestSync = () => invoke<void>('lt_request_sync');

export const onLtState = (cb: (s: LtState) => void): Promise<UnlistenFn> =>
	listen<LtState>('lt-state', (e) => cb(e.payload));
export const onLtNotice = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<string>('lt-notice', (e) => cb(e.payload));
