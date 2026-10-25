// Client-side cache of played YouTube audio — not the user's own local-music library
// ($lib/local/db.ts, a different IndexedDB database entirely, with different lifecycle rules: that
// one is permanent user data, this one is an evictable cache). Every track played gets downloaded
// in full in the background (not blocking playback, which keeps streaming from the server the
// normal way the first time) and stored as a Blob; a replay serves straight from here with no
// network round-trip at all once cached.
//
// Deliberately never re-resolves once cached: the whole point is that a replay plays the exact
// same bytes every time, with no dependency on the resolve/stream pipeline staying up or on a
// re-resolve picking a different stream variant. Metadata (title/artist/thumbnail) for a
// cache-hit replay comes from the queue item itself, not a fresh server response — see
// `web-playback.svelte.ts`'s `urlsFor`.
//
// `pinned` (mint-ui-redesign, specs/offline-downloads) turns an entry from this implicit,
// LRU-evictable play-cache into an explicit "download": `evictIfOverBudget` skips it, so it stays
// until the user removes it, unlike every other entry here. A track only ever becomes pinned
// through an explicit user action (`pin`) — normal playback (`cacheInBackground`) never sets it,
// so "played recently" and "downloaded on purpose" stay genuinely different things.
import { openDB, type DBSchema, type IDBPDatabase } from 'idb';

interface CacheEntry {
	videoId: string;
	blob: Blob;
	bytes: number;
	lastAccessed: number;
	pinned?: boolean;
	/** Only ever set by `pin` — the implicit play-cache path has no reason to carry display
	 *  metadata, since a cache-hit replay already gets it from the queue item (see header comment).
	 *  The Downloads page has no queue item to fall back on, so a pinned entry carries its own. */
	title?: string;
	artist?: string;
	thumbnail?: string;
}

interface YtCacheDB extends DBSchema {
	tracks: { key: string; value: CacheEntry; indexes: { lastAccessed: number } };
}

/** Soft cap on total cache size. Past this, the least-recently-played tracks are evicted first. */
const MAX_BYTES = 1024 * 1024 * 1024; // 1 GiB

let dbPromise: Promise<IDBPDatabase<YtCacheDB>> | null = null;

function db() {
	if (!dbPromise) {
		dbPromise = openDB<YtCacheDB>('limusic-yt-cache', 1, {
			upgrade(d) {
				const store = d.createObjectStore('tracks', { keyPath: 'videoId' });
				store.createIndex('lastAccessed', 'lastAccessed');
			}
		});
	}
	return dbPromise;
}

/** The cached blob for `videoId`, or `null` on a cache miss. Touches the entry's `lastAccessed`
 *  (fire-and-forget) so it survives LRU eviction a while longer. */
export async function getCached(videoId: string): Promise<Blob | null> {
	const d = await db();
	const entry = await d.get('tracks', videoId);
	if (!entry) return null;
	void d.put('tracks', { ...entry, lastAccessed: Date.now() });
	return entry.blob;
}

/** Whether `videoId` is a download (not just an ordinary play-cache hit) — the download-button
 *  toggle in the track menu reads this to show "Download" vs. "Remove download". */
export async function isDownloaded(videoId: string): Promise<boolean> {
	const entry = await (await db()).get('tracks', videoId);
	return !!entry?.pinned;
}

const inFlight = new Set<string>();

/** Shared by `cacheInBackground` (fire-and-forget) and `ensureCached` (awaited, for an explicit
 *  download) — fetches `streamUrl` in full and stores it, unless something's already there. Never
 *  touches `pinned`: whether an entry counts as a download is `pin`'s job alone, not a side effect
 *  of however the bytes got fetched. */
async function fetchAndStore(videoId: string, streamUrl: string): Promise<void> {
	const d = await db();
	const existing = await d.get('tracks', videoId);
	if (existing) return;
	const res = await fetch(streamUrl, { credentials: 'include' });
	if (!res.ok) throw new Error(`download failed: ${res.status}`);
	const blob = await res.blob();
	await d.put('tracks', { videoId, blob, bytes: blob.size, lastAccessed: Date.now() });
}

/** Download the full track from `streamUrl` (our own `/media/stream/:videoId` proxy — a plain GET
 *  with no `Range` header gets the whole file, the same route playback already streams from) and
 *  cache it. Fire-and-forget: never blocks or fails playback, which is already underway on the
 *  normal streaming path by the time this is called. No-op if already cached or a download for
 *  this videoId is already running (a fast skip-forward during the first play, say). */
export function cacheInBackground(videoId: string, streamUrl: string): void {
	if (inFlight.has(videoId)) return;
	inFlight.add(videoId);
	fetchAndStore(videoId, streamUrl)
		.then(() => evictIfOverBudget())
		.catch(() => {
			// Best-effort: a failed background download just means the next play streams again
			// and tries to cache again, not a user-facing failure.
		})
		.finally(() => inFlight.delete(videoId));
}

/** Awaited counterpart for an explicit download (mint-ui-redesign, specs/offline-downloads): the
 *  caller needs to know when the bytes actually land (to show progress/completion/an error), unlike
 *  the normal autoplay path above. No-op if already cached — `pin` is what actually marks it a
 *  download; this only guarantees the bytes exist first. */
export async function ensureCached(videoId: string, streamUrl: string): Promise<void> {
	if (inFlight.has(videoId)) {
		// A background cache for this track is already running — wait for IndexedDB to actually
		// have it rather than racing a second fetch of the same bytes.
		while (inFlight.has(videoId)) await new Promise((r) => setTimeout(r, 50));
		return;
	}
	inFlight.add(videoId);
	try {
		await fetchAndStore(videoId, streamUrl);
	} finally {
		inFlight.delete(videoId);
	}
}

async function evictIfOverBudget() {
	const d = await db();
	// Pinned entries are downloads, not cache: they're exempt from the budget entirely, the same
	// way they're exempt from eviction below — a 1 GiB's worth of downloads plus more on top of it
	// is accepted scope (design.md Risks), not a bug to work around here.
	const oldestFirst = (await d.getAllFromIndex('tracks', 'lastAccessed')).filter((e) => !e.pinned);
	let total = oldestFirst.reduce((sum, e) => sum + e.bytes, 0);
	for (const entry of oldestFirst) {
		if (total <= MAX_BYTES) break;
		await d.delete('tracks', entry.videoId);
		total -= entry.bytes;
	}
}

/** Mark a track a download: it survives `evictIfOverBudget` from here on, and carries its own
 *  display metadata (the Downloads page has no queue item to read it from). The track must already
 *  be cached — callers download it first via `ensureCached`/`cacheInBackground` having already run.
 *  Throws if it isn't, rather than silently caching nothing: a "download" that turns out not to
 *  have downloaded anything is exactly the bug this two-step split exists to avoid. */
export async function pin(
	videoId: string,
	meta: { title: string; artist: string; thumbnail?: string }
): Promise<void> {
	const d = await db();
	const entry = await d.get('tracks', videoId);
	if (!entry) throw new Error('pin: track is not cached yet — call ensureCached first');
	await d.put('tracks', { ...entry, pinned: true, ...meta });
}

/** Remove a download. This deletes the entry outright rather than just clearing `pinned`
 *  (specs/offline-downloads: removing a download must free its storage immediately and require a
 *  network fetch to play again — leaving the blob behind as an ordinary cache entry would do
 *  neither until some later eviction happened to pick it). */
export async function unpin(videoId: string): Promise<void> {
	await forget(videoId);
}

/** Every downloaded track, for the Downloads page. */
export async function listDownloaded(): Promise<
	Array<{ videoId: string; title: string; artist: string; thumbnail?: string; bytes: number }>
> {
	const d = await db();
	const all = await d.getAll('tracks');
	return all
		.filter((e): e is CacheEntry & { pinned: true; title: string; artist: string } => !!e.pinned)
		.map((e) => ({ videoId: e.videoId, title: e.title, artist: e.artist, thumbnail: e.thumbnail, bytes: e.bytes }));
}

/** Total bytes used by downloads specifically (not the whole play-cache) — the Downloads page's
 *  storage-used figure. */
export async function downloadedBytes(): Promise<number> {
	const downloaded = await listDownloaded();
	return downloaded.reduce((sum, e) => sum + e.bytes, 0);
}

/** Forget one cached track (e.g. if it turns out to be corrupt/unplayable) so the next play
 *  re-downloads it fresh. */
export async function forget(videoId: string): Promise<void> {
	await (await db()).delete('tracks', videoId);
}
