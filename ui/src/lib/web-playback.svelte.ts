// Client-owned queue + transport for the browser/PWA build (openspec/changes/http-audio-streaming,
// design.md D5). On desktop, `AppState`'s orchestrator/player own the queue and push it to the
// frontend over Tauri events; here there is no server-side player at all — the browser *is* the
// player, so this module owns the queue directly and writes straight into the shared `playback`/
// `np` state from `player.svelte.ts`, the same way `$lib/local/store.svelte.ts` already does for
// local files. `api.web.ts`'s transport/queue stubs are thin wrappers calling into this module.
import { AudioEngine } from './engine/audio-engine';
import { bindMediaSession, setNowPlayingMeta } from './engine/mediasession';
import { np, playback, toast } from './player.svelte';
import * as lt from './lt-client';
import type { PlaybackWire } from './lt-client';
import * as ytCache from './yt-cache';
import type { NowPlaying, QueueState, PlaybackSnapshot, RepeatMode, SongItem, LtTrack } from './api.types';

// Same split as api.web.ts's SERVER_URL — see its comment.
const SERVER_URL = import.meta.env.DEV ? 'http://localhost:8787' : '';

interface ResolveResponse {
	video_id: string;
	stream_url: string;
	video_url: string | null;
	itag: number;
	expires_in_seconds: number;
	loudness_db: number | null;
	title: string | null;
	artists: string | null;
	duration: string | null;
	thumbnail: string | null;
	is_video: boolean | null;
	stream_client: string;
}

async function resolvePlayback(videoId: string, isUpload?: boolean): Promise<ResolveResponse> {
	const params = new URLSearchParams({ videoId });
	if (isUpload) params.set('isUpload', 'true');
	const res = await fetch(`${SERVER_URL}/api/playback/resolve?${params}`, { credentials: 'include' });
	if (!res.ok) {
		// The server's own error text is already a clear, human-readable sentence (`ResolveError`'s
		// Display impl — "no client could resolve a playable stream for X", "sign in to play this",
		// etc.) — surface it as-is rather than wrapping it in a "resolve failed: 422" prefix.
		const text = await res.text().catch(() => '');
		throw new Error(text || `resolve failed (${res.status})`);
	}
	return res.json();
}

// --- queue state ------------------------------------------------------------------------------

let queue: SongItem[] = [];
let currentIndex = -1;
let playedFrom = 0;
let shuffleOn = false;
let repeatMode: RepeatMode = 'off';
let sourceName: string | null = null;
let sourceId: string | null = null;
/** One-slot stash for `backToPrevious` — the queue a click just replaced. */
let prevQueueSnapshot: { queue: SongItem[]; index: number; sourceName: string | null; sourceId: string | null } | null = null;

let resolvedVideoUrl: string | null = null;
let resolveGeneration = 0;
/** What `preloadNext` last resolved and handed to `AudioEngine.preload` — lets a manual skip reuse
 *  it (`AudioEngine.usePreloaded`) instead of re-running the ~1s cipher/BotGuard/client-fallback
 *  resolve pipeline for a URL that would come back identical anyway (`media.rs`'s resolve always
 *  returns the same deterministic `/media/stream/{video_id}` path). `null` whenever there's nothing
 *  usable: no next item, a cache hit (no `ResolveResponse` to stash — see `ResolvedUrls.resp`), or
 *  the resolve itself failed. */
let preloaded: { videoId: string; resp: ResolveResponse; video: string | null } | null = null;

function current(): SongItem | undefined {
	return queue[currentIndex];
}

function syncQueueState() {
	const state: QueueState = {
		items: queue,
		currentIndex,
		playedFrom,
		shuffle: shuffleOn,
		repeat: repeatMode,
		sourceName,
		sourceId,
		prevTrack: prevQueueSnapshot ? (prevQueueSnapshot.queue[prevQueueSnapshot.index]?.title ?? null) : null
	};
	playback.queue = state;
}

function toNowPlaying(item: SongItem, streamClient: string, isVideo?: boolean | null): NowPlaying {
	return {
		videoId: item.video_id,
		title: item.title,
		artists: item.artists,
		artistId: item.artist_id,
		artistRuns: item.artist_runs,
		thumbnail: item.thumbnail,
		duration: item.duration,
		album: item.album ?? null,
		streamClient,
		rating: item.rating ?? 'indifferent',
		isVideo: isVideo ?? item.is_video
	};
}

/** `SongItem` (snake_case, this app's own shape) → `listen_protocol::Track` (also snake_case —
 *  the wire protocol's own serde config, not a coincidence). Listen Together host broadcast only
 *  (design.md D6). */
function songToLtTrack(item: SongItem): LtTrack {
	return {
		id: item.video_id,
		title: item.title,
		artist: item.artists,
		thumbnail: item.thumbnail,
		duration_ms: item.duration ? parseDurationMs(item.duration) : 0
	};
}

/** `SongItem.duration` is already a display string ("3:45"), not seconds — parse `m:ss`/`h:mm:ss`. */
function parseDurationMs(duration: string): number {
	const parts = duration.split(':').map(Number);
	if (parts.some(Number.isNaN)) return 0;
	const secs = parts.reduce((acc, p) => acc * 60 + p, 0);
	return secs * 1000;
}

function broadcast(p: Partial<PlaybackWire> & Pick<PlaybackWire, 'kind'>) {
	lt.broadcastPlayback({ position_ms: 0, server_time_ms: 0, playing: !playback.paused, volume: playback.volume / 100, ...p });
}

// --- engine -------------------------------------------------------------------------------------

let engine: AudioEngine | null = null;

function getEngine(): AudioEngine {
	if (!engine) {
		engine = new AudioEngine();
		engine.addEventListener('position', (e) => {
			playback.position = (e as CustomEvent).detail.seconds;
			playback.positionAt = performance.now();
		});
		engine.addEventListener('duration', (e) => {
			playback.duration = (e as CustomEvent).detail.seconds;
		});
		engine.addEventListener('playstate', (e) => {
			const playing = (e as CustomEvent).detail.playing as boolean;
			playback.paused = !playing;
			broadcast({ kind: playing ? 'play' : 'pause', position_ms: Math.round(playback.position * 1000) });
		});
		engine.addEventListener('advanced', () => void onAdvanced());
		engine.addEventListener('ended', () => void onEndedWithoutPreload());
		engine.addEventListener('error', () => void onTrackFailed());
		bindMediaSession(engine, { next: () => void nextTrack(), prev: () => void prevTrack() });
	}
	return engine;
}

interface ResolvedUrls {
	stream: string;
	video: string | null;
	/** `null` on a cache hit — a replay never re-resolves, so there's no fresh server response to
	 *  read metadata from; the caller falls back to the queue item's own fields (see `playCurrent`). */
	resp: ResolveResponse | null;
	/** Whether `stream` is a local `blob:` URL (cached) rather than a server URL — tells
	 *  `AudioEngine.loadTrack`/`preload` to revoke it on replacement, same as local-music playback. */
	isObjectUrl: boolean;
}

async function urlsFor(item: SongItem): Promise<ResolvedUrls> {
	const cached = await ytCache.getCached(item.video_id);
	if (cached) {
		return { stream: URL.createObjectURL(cached), video: null, resp: null, isObjectUrl: true };
	}
	const resp = await resolvePlayback(item.video_id, item.is_upload);
	const stream = `${SERVER_URL}${resp.stream_url}`;
	// Fire-and-forget: downloads the full file in the background and caches it, so the *next*
	// play of this track skips the network/resolve pipeline entirely (yt-cache.ts's own header
	// comment has the full reasoning). Never blocks or delays the playback already underway below.
	ytCache.cacheInBackground(item.video_id, stream);
	return { stream, video: resp.video_url ? `${SERVER_URL}${resp.video_url}` : null, resp, isObjectUrl: false };
}

async function playCurrent(autoplay: boolean) {
	const item = current();
	if (!item) return;
	const gen = ++resolveGeneration;
	const engine = getEngine();
	try {
		let video: string | null;
		let resp: ResolveResponse | null;
		const pending = preloaded;
		if (pending && pending.videoId === item.video_id && (await engine.usePreloaded({ autoplay }))) {
			resp = pending.resp;
			video = pending.video;
		} else {
			const resolved = await urlsFor(item);
			if (gen !== resolveGeneration) return; // superseded by a newer play before this resolved
			await engine.loadTrack(resolved.stream, { autoplay, objectUrl: resolved.isObjectUrl });
			resp = resolved.resp;
			video = resolved.video;
		}
		if (gen !== resolveGeneration) return;
		preloaded = null;
		resolvedVideoUrl = video;
		playback.now = toNowPlaying(item, resp?.stream_client ?? 'cache', resp?.is_video);
		playback.rating = item.rating ?? 'indifferent';
		setNowPlayingMeta({
			title: item.title,
			artist: item.artists,
			album: item.album,
			artwork: item.thumbnail
		});
		broadcast({
			kind: 'change_track',
			track: songToLtTrack(item),
			queue: queue.slice(currentIndex + 1).map(songToLtTrack),
			playing: autoplay
		});
		void preloadNext();
	} catch (e) {
		if (gen !== resolveGeneration) return;
		playback.now = toNowPlaying(item, 'error');
		playback.paused = true;
		// Previously silent: the position/duration stayed at 0:00 with no indication why.
		// `resolvePlayback`'s own Error already carries the server's real reason (e.g. every
		// client exhausted, sign-in required) — surface it instead of failing invisibly.
		toast.error(e instanceof Error ? e.message : String(e));
	}
	syncQueueState();
}

async function preloadNext() {
	const next = queue[currentIndex + 1];
	preloaded = null;
	if (!next) return;
	try {
		const { stream, video, resp, isObjectUrl } = await urlsFor(next);
		getEngine().preload(stream, { objectUrl: isObjectUrl });
		// A cache hit has no ResolveResponse to stash (see ResolvedUrls.resp) — a manual skip onto
		// it still gets the engine-level reuse (AudioEngine.preloadedUrl), just not the resolve-skip.
		if (resp) preloaded = { videoId: next.video_id, resp, video };
	} catch {
		// Falls back to a normal full load in `onEndedWithoutPreload` once playback gets there.
	}
}

function nextIndexFor(from: number): number | null {
	if (from + 1 < queue.length) return from + 1;
	if (repeatMode === 'all' && queue.length > 0) return 0;
	return null;
}

async function onAdvanced() {
	const next = repeatMode === 'one' ? currentIndex : nextIndexFor(currentIndex);
	if (next === null) return;
	currentIndex = next;
	const item = current();
	if (item) {
		playback.now = toNowPlaying(item, playback.now?.streamClient ?? 'unknown');
		playback.rating = item.rating ?? 'indifferent';
		setNowPlayingMeta({ title: item.title, artist: item.artists, album: item.album, artwork: item.thumbnail });
	}
	syncQueueState();
	void preloadNext();
}

async function onEndedWithoutPreload() {
	const next = repeatMode === 'one' ? currentIndex : nextIndexFor(currentIndex);
	if (next === null) return;
	currentIndex = next;
	await playCurrent(true);
}

async function onTrackFailed() {
	// Skip a track that failed to resolve/play rather than stall the queue on it.
	const next = nextIndexFor(currentIndex);
	if (next === null) return;
	currentIndex = next;
	await playCurrent(true);
}

function stashCurrentQueue() {
	if (queue.length === 0) return;
	prevQueueSnapshot = { queue, index: currentIndex, sourceName, sourceId };
}

// --- public API, matched against the `api.*` transport/queue surface --------------------------

export function getQueue(): QueueState {
	return {
		items: queue,
		currentIndex,
		playedFrom,
		shuffle: shuffleOn,
		repeat: repeatMode,
		sourceName,
		sourceId
	};
}

export function getPlayback(): PlaybackSnapshot {
	return {
		now: playback.now,
		paused: playback.paused,
		position: playback.position,
		duration: playback.duration,
		volume: playback.volume / 100
	};
}

export async function play(item: SongItem): Promise<void> {
	// Must happen before the first `await` below — see `AudioEngine.primeForGesture`'s own comment.
	getEngine().primeForGesture();
	stashCurrentQueue();
	queue = [item];
	currentIndex = 0;
	playedFrom = 0;
	sourceName = null;
	sourceId = null;
	await playCurrent(true);
}

export async function playIndex(index: number): Promise<void> {
	if (index < 0 || index >= queue.length) return;
	getEngine().primeForGesture();
	currentIndex = index;
	await playCurrent(true);
}

export function playNext(items: SongItem[], from?: string): void {
	const tagged = items.map((i) => ({ ...i, queued: true, queued_from: from }));
	const insertAt = currentIndex + 1;
	queue = [...queue.slice(0, insertAt), ...tagged, ...queue.slice(insertAt)];
	syncQueueState();
}

export function addToQueue(items: SongItem[], from?: string): void {
	const tagged = items.map((i) => ({ ...i, queued_end: true, queued_from: from }));
	queue = [...queue, ...tagged];
	syncQueueState();
}

export function clearQueued(): void {
	queue = queue.filter((i, idx) => idx <= currentIndex || (!i.queued && !i.queued_end));
	syncQueueState();
}

export function removeFromQueue(index: number): void {
	if (index <= currentIndex) return;
	queue = [...queue.slice(0, index), ...queue.slice(index + 1)];
	syncQueueState();
}

export function moveInQueue(from: number, to: number): void {
	if (from <= currentIndex || to <= currentIndex) return;
	const items = [...queue];
	const [moved] = items.splice(from, 1);
	items.splice(to, 0, moved);
	queue = items;
	syncQueueState();
}

export async function nextTrack(): Promise<void> {
	const next = nextIndexFor(currentIndex);
	if (next === null) return;
	getEngine().primeForGesture();
	currentIndex = next;
	await playCurrent(true);
}

export async function prevTrack(): Promise<void> {
	// From the top of a track: restart it. More than a couple seconds in: restart, don't skip back
	// (matches the desktop's own "prev restarts unless near the very start" convention).
	if (playback.position > 3 || currentIndex <= 0) {
		getEngine().seek(0);
		return;
	}
	getEngine().primeForGesture();
	currentIndex -= 1;
	await playCurrent(true);
}

export function backToPrevious(): void {
	if (!prevQueueSnapshot) return;
	queue = prevQueueSnapshot.queue;
	currentIndex = prevQueueSnapshot.index;
	sourceName = prevQueueSnapshot.sourceName;
	sourceId = prevQueueSnapshot.sourceId;
	prevQueueSnapshot = null;
	void playCurrent(true);
}

export function toggleShuffle(): void {
	shuffleOn = !shuffleOn;
	syncQueueState();
}

export function setRepeat(mode: RepeatMode): void {
	repeatMode = mode;
	syncQueueState();
}

export function togglePause(): void {
	getEngine().toggle();
}

export function seek(position: number): void {
	getEngine().seek(position);
	broadcast({ kind: 'seek', position_ms: Math.round(position * 1000) });
}

export function setVolume(volume: number): void {
	playback.volume = volume;
	getEngine().setVolume(volume / 100);
}

export async function playPlaylist(
	items: SongItem[],
	start: number | null,
	sourceIdArg?: string,
	sourceNameArg?: string,
	shuffle?: boolean
	// `continuation` (background-walking the rest of a long playlist into the queue) is not wired
	// here — a real but bounded scope cut: every item the caller already fetched plays, just not a
	// playlist's later pages beyond what the page view itself loaded. A later change can add it.
): Promise<void> {
	getEngine().primeForGesture();
	stashCurrentQueue();
	let ordered = items;
	let startIndex = start ?? 0;
	if (shuffle) {
		const head = items[startIndex];
		const rest = items.filter((_, i) => i !== startIndex);
		for (let i = rest.length - 1; i > 0; i--) {
			const j = Math.floor(Math.random() * (i + 1));
			[rest[i], rest[j]] = [rest[j], rest[i]];
		}
		ordered = head ? [head, ...rest] : rest;
		startIndex = 0;
	}
	queue = ordered;
	currentIndex = startIndex;
	playedFrom = startIndex;
	sourceId = sourceIdArg ?? null;
	sourceName = sourceNameArg ?? null;
	await playCurrent(true);
}

/** The current track's music-video URL (already resolved alongside the audio), for `VideoSurface`.
 *  `null` when the track has none, or the audio resolve hasn't completed yet. */
export function currentVideoUrl(): string | null {
	return resolvedVideoUrl;
}
