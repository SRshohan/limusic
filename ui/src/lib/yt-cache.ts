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
import { openDB, type DBSchema, type IDBPDatabase } from 'idb';

interface CacheEntry {
	videoId: string;
	blob: Blob;
	bytes: number;
	lastAccessed: number;
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

const inFlight = new Set<string>();

/** Download the full track from `streamUrl` (our own `/media/stream/:videoId` proxy — a plain GET
 *  with no `Range` header gets the whole file, the same route playback already streams from) and
 *  cache it. Fire-and-forget: never blocks or fails playback, which is already underway on the
 *  normal streaming path by the time this is called. No-op if already cached or a download for
 *  this videoId is already running (a fast skip-forward during the first play, say). */
export function cacheInBackground(videoId: string, streamUrl: string): void {
	if (inFlight.has(videoId)) return;
	inFlight.add(videoId);
	run().finally(() => inFlight.delete(videoId));

	async function run() {
		const d = await db();
		if (await d.get('tracks', videoId)) return;
		try {
			const res = await fetch(streamUrl, { credentials: 'include' });
			if (!res.ok) return;
			const blob = await res.blob();
			await d.put('tracks', { videoId, blob, bytes: blob.size, lastAccessed: Date.now() });
			await evictIfOverBudget();
		} catch {
			// Best-effort: a failed background download just means the next play streams again
			// and tries to cache again, not a user-facing failure.
		}
	}
}

async function evictIfOverBudget() {
	const d = await db();
	const oldestFirst = await d.getAllFromIndex('tracks', 'lastAccessed');
	let total = oldestFirst.reduce((sum, e) => sum + e.bytes, 0);
	for (const entry of oldestFirst) {
		if (total <= MAX_BYTES) break;
		await d.delete('tracks', entry.videoId);
		total -= entry.bytes;
	}
}

/** Forget one cached track (e.g. if it turns out to be corrupt/unplayable) so the next play
 *  re-downloads it fresh. */
export async function forget(videoId: string): Promise<void> {
	await (await db()).delete('tracks', videoId);
}
