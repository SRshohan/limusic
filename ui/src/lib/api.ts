// The UI's only door to the backend. Picks the Tauri (`api.tauri.ts`) or web (`api.web.ts`)
// command implementation once, here, at module load — so neither the UI components nor the two
// implementation modules ever need an `isTauri` branch of their own. See platform.ts for the flag
// and api.types.ts for the shared data model both implementations speak.
import { isTauri } from './platform';
import * as tauriApi from './api.tauri';
import * as webApi from './api.web';

export * from './api.types';

const impl: typeof tauriApi = isTauri ? tauriApi : webApi;

/** Web's one browser login path (cookie-paste) — no Tauri-side equivalent command at all (desktop
 *  uses `loginWebview`'s native Google sign-in instead), so this isn't part of the shared `impl`
 *  surface above; it's just unreachable on desktop rather than pretending to be cross-platform. */
export const signInWithCookie: typeof webApi.signInWithCookie = isTauri
	? () => Promise.reject(new Error('signInWithCookie: desktop uses the native Google sign-in instead'))
	: webApi.signInWithCookie;

/** Explicit offline downloads (mint-ui-redesign, specs/offline-downloads) are a browser-storage
 *  (`yt-cache.ts`/IndexedDB) concept with no Tauri-side equivalent at all — desktop already caches
 *  every stream to disk via mpv itself, unconditionally, so there's nothing here for it to opt into.
 *  Unreachable on desktop (the Downloads page/download action are web-only UI) rather than
 *  pretending to be cross-platform. */
const downloadUnsupported = () =>
	Promise.reject(new Error('offline downloads: desktop already caches every stream to disk'));
export const downloadTrack: typeof webApi.downloadTrack = isTauri ? downloadUnsupported : webApi.downloadTrack;
export const removeDownload: typeof webApi.removeDownload = isTauri ? downloadUnsupported : webApi.removeDownload;
export const isDownloaded: typeof webApi.isDownloaded = isTauri
	? () => Promise.resolve(false)
	: webApi.isDownloaded;
export const listDownloaded: typeof webApi.listDownloaded = isTauri
	? () => Promise.resolve([])
	: webApi.listDownloaded;
export const downloadedBytes: typeof webApi.downloadedBytes = isTauri
	? () => Promise.resolve(0)
	: webApi.downloadedBytes;

export const {
	// commands (context/11)
	search,
	searchVideos,
	searchAll,
	searchSuggestions,
	searchCards,
	play,
	playIndex,
	removeFromQueue,
	moveInQueue,
	playNext,
	addToQueue,
	clearQueued,
	nextTrack,
	prevTrack,
	backToPrevious,
	toggleShuffle,
	setRepeat,
	togglePause,
	seek,
	setVolume,
	setPlaybackParams,
	getQueue,
	videoStream,
	forgetVideoStream,
	nativeVideoRect,
	ambientFrame,
	getPlayback,
	// settings
	getSettings,
	setSetting,
	getStreamClients,
	clearCaches,
	setAppIcon,
	appIconPath,
	allowFontFile,
	// global hotkeys
	getGlobalHotkeys,
	globalHotkeysOnWayland,
	setGlobalHotkeys,
	resetGlobalHotkeys,
	releaseNotes,
	canSelfUpdate,
	checkBetaUpdate,
	openExternal,
	diagnostics,
	diagnosticsSummary,
	saveDiagnostics,
	// auth (context/15)
	getAccount,
	getAccountIdentities,
	switchAccount,
	signOut,
	loginWebview,
	getGoogleAccounts,
	switchGoogleAccount,
	removeGoogleAccount,
	// mini player
	openMini,
	closeMini,
	setMiniCompact,
	// browse / library (context/08)
	getHome,
	getHomeMore,
	getLibrary,
	getLocalPlaylists,
	getLibraryAlbums,
	getLibraryArtists,
	getUploadAlbums,
	getHistory,
	getPlaylist,
	setPlaylistSort,
	getPlaylistMore,
	playlistIndex,
	syncPlaylistIndex,
	getPlayCounts,
	playPlaylist,
	startRadio,
	getAlbum,
	getArtist,
	getMoods,
	getMoodArt,
	getBrowseGrid,
	// local music (local.rs)
	getLocalLibrary,
	addLocalFolder,
	removeLocalFolder,
	// blocked artists (blocked.rs, plan 046)
	getBlockedArtists,
	blockArtist,
	unblockArtist,
	// write actions (context/01 ✎)
	rate,
	addToPlaylist,
	removeFromPlaylist,
	removeManyFromPlaylist,
	createPlaylist,
	addToLocalPlaylist,
	editPlaylistDetails,
	setPlaylistCover,
	deletePlaylist,
	subscribe,
	setSongSaved,
	setAlbumSaved,
	// events (context/11)
	onNowPlaying,
	onRating,
	onVideoReady,
	onQueueChanged,
	onQueueIndex,
	onQueueAppended,
	onUiVisible,
	takeLaunchArgs,
	onOpenLink,
	onPosition,
	onDuration,
	onVolume,
	onPlaybackState,
	onPlaybackError,
	onPlaybackNotice,
	onCoverError,
	onAuthChanged,
	onAccountSelectionRequired,
	onLocalChanged,
	onLoginError,
	onLoginDone,
	// lyrics
	getLyrics,
	chooseLyricsSource,
	setLyricsOffset,
	lyricsProviders,
	// window
	theaterFullscreen,
	// Last.fm scrobbling
	lastfmStatus,
	lastfmConnect,
	lastfmDisconnect,
	onLastfmState,
	// Listen Together (context/19)
	ltGetState,
	ltSetServerUrl,
	ltCreateRoom,
	ltJoinRoom,
	ltLeave,
	ltApproveJoin,
	ltRejectJoin,
	ltKick,
	ltTransferHost,
	ltApproveSuggestion,
	ltRejectSuggestion,
	ltRequestSync,
	onLtState,
	onLtNotice
} = impl;
