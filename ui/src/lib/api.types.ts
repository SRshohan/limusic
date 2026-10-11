// Shared data model for the UI↔backend boundary. Deliberately platform-agnostic: no `invoke`,
// no `listen`, no `fetch`, no knowledge of Tauri or the web server. `api.tauri.ts` and
// `api.web.ts` each implement the commands below against these same shapes; `api.ts` picks one
// implementation at module load based on `isTauri` (see platform.ts) so neither side has to
// branch per call.

/** What `@tauri-apps/api/event`'s `listen` returns — redeclared here (identical shape) so this
 *  module never has to import a Tauri package just for a type used in a cross-platform signature. */
export type UnlistenFn = () => void;

/** How the signed-in user rated a track (innertube `Rating`). The three states are mutually
 *  exclusive: liking a disliked track clears the dislike, and vice versa. */
export type Rating = 'like' | 'dislike' | 'indifferent';

/** One run of an artist line: its text, plus a channel id when that run links an artist. */
export interface ArtistRun {
	text: string;
	id?: string;
}

export interface SongItem {
	video_id: string;
	title: string;
	artists: string;
	/** Primary artist's channel browseId (`UC…`), when linked — makes the artist name navigable. */
	artist_id?: string;
	/** The artist line run by run — a collab links each name to its own page. Empty/absent when
	 * nothing is linked; render plain `artists` then. */
	artist_runs?: ArtistRun[];
	album?: string;
	/** The album's browseId (`MPRE…`), when linked — makes the album navigable. */
	album_id?: string;
	duration?: string;
	/** Play count as YouTube abbreviates it ("53M"). Album, artist and search rows. */
	play_count?: string;
	thumbnail?: string;
	/** Item id within a playlist — present only on playlist tracks; needed to remove them. */
	set_video_id?: string;
	/** Collaborative playlists only: who added this track, and their avatar. */
	added_by?: string;
	added_by_avatar?: string;
	/** The signed-in user's rating (absent when the response didn't say — same as 'indifferent'). */
	rating?: Rating;
	/** "Add to library" off the row's own menu, with a token for each direction. Absent on rows
	 *  YouTube sent no menu for, and on the ones built here (local files, On Repeat, a Listen
	 *  Together guest's queue) — the menu hides the action rather than offering a dead one.
	 *  Library ▸ Songs is not Liked Music: this is a feedback write, not a rating. */
	library?: { in_library: boolean; add_token?: string; remove_token?: string };
	/** Listen Together: name of the guest who added this queue item (session adds only). */
	queued_by?: string;
	/** Queued to play next ("Play next", or a guest's session add) — the "Next in queue" block. */
	queued?: boolean;
	/** Appended by "Add to queue" — its own block at the tail of the queue. */
	queued_end?: boolean;
	/** The album/playlist either block was added from, for its heading in the queue panel. */
	queued_from?: string;
	/** Appended by autoplay radio continuation — drives the queue's "Autoplay" divider + badge. */
	autoplay?: boolean;
	/** YouTube flags the track explicit. Browse/search rows only: `/next` carries no badge, so a
	 *  radio- or autoplay-appended track arrives without it. */
	explicit?: boolean;
	/** This row links a music video rather than the audio track. */
	is_video?: boolean;
	/** One of the user's own YouTube Music uploads. Set by Rust and passed straight back on play:
	 *  only an authenticated client can stream one, and the row is where that is known. */
	is_upload?: boolean;
}

export interface NowPlaying {
	videoId: string;
	title: string;
	artists: string;
	artistId?: string;
	/** The artist line run by run — links each artist of a collab separately. */
	artistRuns?: ArtistRun[];
	thumbnail?: string;
	duration?: string;
	album?: string | null;
	streamClient: string;
	/** The user's rating of the track (null if unknown). */
	rating?: Rating | null;
	/** YouTube's `musicVideoType` says this is a video upload, not the generated audio track.
	 *  Gates the player view's music-video mode. */
	isVideo?: boolean;
}

export type RepeatMode = 'off' | 'all' | 'one';

export interface QueueState {
	items: SongItem[];
	currentIndex: number;
	/** Start of the previously-played run: `items[playedFrom..currentIndex]` has actually been
	 *  heard. Not `0..currentIndex`: a playlist opened at track 7 has six untouched tracks first. */
	playedFrom?: number;
	shuffle?: boolean;
	repeat?: RepeatMode;
	/** What seeded the queue (playlist/album title, "<song> Radio") — the "Next from" header. */
	sourceName?: string | null;
	/** The playlist the queue was started from, when it was one. What "Remove from this playlist"
	 *  in the player's track menu writes to; absent for radios, single songs and guest queues. */
	sourceId?: string | null;
	/** Title of the track a click replaced this queue mid-play, for the panel's "Back to …" line.
	 *  Set only while `backToPrevious` would actually do something: at the head of the queue, with
	 *  a kept one behind it. */
	prevTrack?: string | null;
}

export interface Account {
	signedIn: boolean;
	name?: string | null;
	handle?: string | null;
	email?: string | null;
	thumbnail?: string | null;
	channelId?: string | null;
	canSwitch?: boolean;
	/** The cookie authenticated, but a multi-channel login is not complete until one is chosen. */
	selectionRequired?: boolean;
}

export interface AccountIdentity {
	/** Opaque, process-local selector. Raw delegated/data-sync ids stay in Rust. */
	selectionKey: string;
	name: string;
	handle?: string | null;
	email?: string | null;
	thumbnail?: string | null;
	channelId?: string | null;
	selected: boolean;
}

/** One saved Google account (multi-account). Display fields only — cookies stay in Rust. */
export interface SavedAccount {
	/** Opaque, process-local selector. */
	id: string;
	name?: string | null;
	handle?: string | null;
	email?: string | null;
	thumbnail?: string | null;
	/** The account whose session is currently driving requests. */
	active: boolean;
}

export interface BrowseItem {
	kind: 'song' | 'playlist' | 'album' | 'artist';
	/** videoId (song) or browseId (playlist/album/artist). */
	id: string;
	title: string;
	subtitle?: string;
	thumbnail?: string;
	/** "3:47" — song items from a list-style shelf only (card shelves don't carry one). */
	duration?: string;
	/** Song cards only: the track's album (`MPRE…`), what puts "Go to album" in its menus. */
	albumId?: string;
	/** Song cards only: the artist line run by run, so a card that gets played keeps its links. */
	artistRuns?: ArtistRun[];
	/** Play count as YouTube abbreviates it ("2.5B") — search song rows only. */
	playCount?: string;
	/** YouTube flags this track/album explicit. */
	explicit?: boolean;
	/** Song cards only: one of the user's own uploads. Carried into the SongItem `asSong` builds,
	 *  because that flag is what picks the login-only client chain when it plays. */
	isUpload?: boolean;
}

export interface HomeSection {
	title: string;
	items: BrowseItem[];
	moreBrowseId?: string;
	moreParams?: string;
}
/** A mood/genre filter chip above the home feed; `params` re-fetches home filtered to it. */
export interface HomeChip {
	title: string;
	params: string;
}
export interface HomePage {
	chips: HomeChip[];
	sections: HomeSection[];
	continuation?: string;
}

/**
 * The On Repeat auto-playlist's synthetic browseId (mirrors `ON_REPEAT_ID` in state.rs). It routes
 * like any other playlist; the only thing the UI does differently is draw an icon cover, because
 * a playlist built from local play counts has no artwork of its own.
 */
export const ON_REPEAT_ID = 'LIMUSIC_ON_REPEAT';

/**
 * Liked Music's browseId. YouTube edits this one through the rating endpoint, not `edit_playlist`,
 * so it is never an add/remove/rename target: liking the song is the edit.
 */
export const LIKED_MUSIC_ID = 'VLLM';

/**
 * YouTube Music's own Library ▸ Songs, despite the name: the songs saved to the account's library.
 * It browses like a playlist (no header, no sort menu), so `getPlaylist` reads it and the Library
 * page's Songs tab pages through it with `getPlaylistMore`.
 */
export const LIBRARY_SONGS_ID = 'FEmusic_liked_videos';

/**
 * The tracks the signed-in user uploaded to YouTube Music themselves. Browses like the songs grid
 * above, so the same tab component reads it; the rows come back with `is_upload` set, which is what
 * sends them down the login-only fallback chain when they play (issue #71).
 */
export const LIBRARY_UPLOADS_ID = 'FEmusic_library_privately_owned_tracks';

/**
 * Local music (Rust `local.rs`). A file on disk is a song whose `video_id` is `LOCAL:<path>`, and
 * an album of them is a browseId `LOCALALBUM:<key>` — so local items ride every existing surface
 * (cards, queue, Shortcuts, the album page) and play with no network.
 */
export const LOCAL_SONG_PREFIX = 'LOCAL:';
export const LOCAL_ALBUM_PREFIX = 'LOCALALBUM:';
/** An artist on this disk. Renders through the album route: same page, no YouTube channel. */
export const LOCAL_ARTIST_PREFIX = 'LOCALARTIST:';
/**
 * A playlist kept on this machine, no account needed (#251; mirrors `LOCAL_PLAYLIST_PREFIX` in
 * state.rs). The playlist commands answer it from SQLite, so it rides the same route and the same
 * calls as a YouTube playlist, and can hold local files as well as YouTube tracks.
 */
export const LOCAL_PLAYLIST_PREFIX = 'LOCALPLAYLIST:';
export const isLocalPlaylist = (id: string | undefined | null): boolean =>
	!!id && id.startsWith(LOCAL_PLAYLIST_PREFIX);
/** Anything with no YouTube item behind it: nothing to share, no radio, no account to save it to. */
export const isLocalId = (id: string | undefined | null): boolean =>
	!!id &&
	(id.startsWith(LOCAL_SONG_PREFIX) ||
		id.startsWith(LOCAL_ALBUM_PREFIX) ||
		id.startsWith(LOCAL_ARTIST_PREFIX) ||
		id.startsWith(LOCAL_PLAYLIST_PREFIX));

export interface LocalLibrary {
	/** Watched folders, as absolute paths. */
	folders: string[];
	albums: BrowseItem[];
	artists: BrowseItem[];
	songs: SongItem[];
	/** Song/album/artist ids that were in the library but are gone from disk since the last scan. */
	removed: string[];
}

/** The orders YouTube itself can put a playlist in — everything in `SortKey` but our own `plays`. */
export type ServerSort = 'default' | 'newest' | 'oldest' | 'title' | 'artist' | 'album' | 'top';

export interface SortMenu {
	/** The order YouTube has this list in right now, when it is one we have a name for. */
	selected?: ServerSort;
	/**
	 * The choice is a write, so storing it makes YouTube Music and every other client follow.
	 * Playlists you own only: elsewhere the menu is view-only (Liked Music remembers the last order
	 * asked for anyway, someone else's playlist does not).
	 */
	editable: boolean;
}

/** One day bucket of the play history: YouTube's own heading plus that day's rows. */
export interface HistoryGroup {
	title: string;
	items: SongItem[];
}

export interface PlaylistPage {
	title?: string;
	subtitle?: string;
	thumbnail?: string;
	/** The playlist's own blurb, which the edit dialog prefills its description with. */
	description?: string;
	/** `PUBLIC` / `PRIVATE` / `UNLISTED`. Only playlists you own report it. */
	privacy?: string;
	/** Custom artwork picked on this machine; falls back to `thumbnail` when unset. */
	cover?: string;
	items: SongItem[];
	continuation?: string;
	/** True only when the signed-in user owns this playlist (rename/delete allowed). */
	owned: boolean;
	/** Collaboration is on: others can add to it, and each person may remove only what they added. */
	collaborative: boolean;
	/** Absent on lists YouTube will not reorder: albums, its own radio mixes, On Repeat. */
	sortMenu?: SortMenu;
}
export interface PlaylistContinuation {
	items: SongItem[];
	continuation?: string;
}

export interface ArtistCarousel {
	title: string;
	items: BrowseItem[];
	moreBrowseId?: string;
	moreParams?: string;
}
export interface SearchResults {
	top: BrowseItem[];
	songs: BrowseItem[];
	albums: BrowseItem[];
	artists: BrowseItem[];
	playlists: BrowseItem[];
}

/** Typeahead under a search field: query completions, then a few matching rows. */
export interface SearchSuggestions {
	queries: { text: string; /** One of the account's own past searches. */ history: boolean }[];
	items: BrowseItem[];
}

export interface AlbumPage {
	title?: string;
	artist?: string;
	artistId?: string;
	/** The artist line run by run — links each artist of a collaborative album separately. */
	artistRuns?: ArtistRun[];
	artistThumbnail?: string;
	subtitle?: string;
	secondSubtitle?: string;
	description?: string;
	thumbnail?: string;
	items: SongItem[];
	continuation?: string;
	/** The album itself is flagged explicit (the header wears the badge, not just some tracks). */
	explicit?: boolean;
	/** The album's audio playlist id (`OLAK5uy_…`) — autoplay's radio seed, and the save target. */
	playlistId?: string;
	/** Already saved to the signed-in user's library. */
	inLibrary: boolean;
	/** Card shelves under the tracks: other versions, more from the artist, related releases. */
	sections?: ArtistCarousel[];
}

export interface ArtistPage {
	name?: string;
	thumbnail?: string;
	description?: string;
	subscribers?: string;
	monthlyListeners?: string;
	channelId: string;
	subscribed: boolean;
	topSongs: SongItem[];
	/** `VL…` playlist of all the artist's top songs, behind the shelf's "See all". */
	topSongsId?: string;
	sections: ArtistCarousel[];
}

/** What the event stream already reported, for a webview that started after it did. */
export interface PlaybackSnapshot {
	now: NowPlaying | null;
	paused: boolean;
	position: number;
	duration: number;
	/** The level restored from last run (or the one another window already set). */
	volume: number;
}

export interface HotkeysConfig {
	enabled: boolean;
	bindings: Record<string, string>;
}

export interface HotkeyRegisterResult {
	success: boolean;
	config: HotkeysConfig;
	errors: Record<string, string>;
}

/** One published release: the GitHub release description, verbatim markdown. */
export interface ReleaseNote {
	version: string;
	/** `YYYY-MM-DD` */
	date: string;
	body: string;
}

/** A Moods & Genres tile. `params` browses `MOODS_CATEGORY_ID` into that mood's playlists. */
export interface Mood {
	title: string;
	params: string;
	/** YouTube's own colour for the tile, `#rrggbb`. */
	color: string;
}
export interface MoodSection {
	title: string;
	items: Mood[];
}
export const MOODS_CATEGORY_ID = 'FEmusic_moods_and_genres_category';

/** One entry in the block list. `id` is the channel browseId when the blocked row linked one. */
export interface BlockedArtist {
	id?: string;
	name: string;
}

/**
 * The queue moved but its track list did not: only the play pointer and the flags changed.
 * Emitted instead of `queue-changed` on every advance and skip, because the full item list is
 * megabytes on a big playlist and a Tauri event delivers its payload as JavaScript *source*.
 * `current` carries the playing row so a metadata backfill (duration, artists) still lands.
 */
export interface QueueIndex {
	currentIndex: number;
	playedFrom?: number;
	shuffle?: boolean;
	repeat?: RepeatMode;
	sourceName?: string | null;
	sourceId?: string | null;
	prevTrack?: string | null;
	current: SongItem | null;
}

/**
 * Autoplay topped the queue up at the tail. Carries only the new rows plus the resulting length, so
 * an endless radio session does not re-ship the whole list (which a Tauri event delivers as
 * JavaScript *source*) every twenty tracks. `len` is the resync guard: if the array we hold does
 * not reach that length once the rows are appended, an event was missed and the panel refetches.
 */
export interface QueueAppended {
	items: SongItem[];
	len: number;
	currentIndex: number;
	playedFrom?: number;
}

export interface LyricWord {
	text: string;
	start_ms: number;
	end_ms: number;
}
export interface LyricLine {
	/** Start cue in milliseconds; present ⇔ the line is synced. */
	time_ms?: number;
	end_time_ms?: number;
	text: string;
	words?: LyricWord[];
	translation?: string;
	/** Latin-script reading of `text` (#202). Word-timed only when Apple Music wrote it. */
	romanized?: string;
	romanized_words?: LyricWord[];
}
export interface Lyrics {
	/** Attribution for the panel footer ("LRCLIB", "Source: Musixmatch", …). */
	source: string;
	/** Id of the provider that answered (`LyricsProvider.id`). */
	provider: string;
	synced: boolean;
	instrumental: boolean;
	lines: LyricLine[];
	/** The source was picked by hand for this song (or its timing nudged). */
	pinned: boolean;
	/** Timing nudge in ms, positive = lyrics later. */
	offset_ms: number;
}
// A type, not an interface: `invoke` takes a record, and only a type alias is assignable to one.
export type LyricsTrack = {
	videoId: string;
	title: string;
	artists: string;
	album?: string;
	duration?: number;
};
export interface LyricsProvider {
	id: string;
	name: string;
	on: boolean;
}

export interface LastfmState {
	connected: boolean;
	username?: string | null;
	/** Set when a connect attempt failed (timeout, network, rejected) — show it as a toast. */
	error?: string | null;
}

export interface LtUser {
	user_id: string;
	username: string;
	is_host: boolean;
	is_connected: boolean;
}
export interface LtTrack {
	id: string;
	title: string;
	artist: string;
	thumbnail?: string | null;
	duration_ms: number;
	/** Name of the guest who added this track to the session queue. */
	queued_by?: string | null;
}
export interface LtPendingJoin {
	userId: string;
	username: string;
}
export interface LtSuggestion {
	id: string;
	from_user_id: string;
	from_username: string;
	track: LtTrack;
}
export interface LtState {
	status: 'disconnected' | 'connecting' | 'connected';
	role: 'none' | 'host' | 'guest';
	/** Asked to create/join and awaiting the room (host approval) — show a waiting state. */
	requesting: boolean;
	roomCode: string | null;
	myId: string | null;
	/** Empty means the built-in default; the backend resolves it when it connects. */
	serverUrl: string;
	/** What that default is. Only ever rendered inside the "change server" panel. */
	defaultServerUrl: string;
	users: LtUser[];
	currentTrack: LtTrack | null;
	queue: LtTrack[];
	pendingJoins: LtPendingJoin[];
	suggestions: LtSuggestion[];
}
