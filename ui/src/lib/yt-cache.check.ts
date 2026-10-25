// Self-check for yt-cache.ts's pinned-download behavior (mint-ui-redesign, specs/offline-downloads).
// Node has no IndexedDB, so this is the one *.check.ts that needs a polyfill — fake-indexeddb
// (devDependency) before anything else runs, so `idb`'s openDB sees a working `indexedDB` global.
//
//     node --experimental-strip-types ui/src/lib/yt-cache.check.ts
//
// Prints "ok" and exits 0, or throws on the first broken invariant. The bug this guards: a pinned
// (downloaded) entry must survive `evictIfOverBudget` even as the least-recently-accessed entry
// over budget — that's the entire point of pinning something, and a regression here would silently
// delete a download the user asked to keep.
import 'fake-indexeddb/auto';

function ok(cond: boolean, what: string): void {
	if (!cond) throw new Error(`FAIL: ${what}`);
}

// Stub fetch: yt-cache.ts only ever calls it with { credentials: 'include' } and reads .ok/.blob().
const bytes = (n: number) => new Blob([new Uint8Array(n)]);
globalThis.fetch = (async (url: string) => {
	const size = Number(new URL(url, 'http://x').searchParams.get('size') ?? '1000');
	return { ok: true, blob: async () => bytes(size) } as Response;
}) as typeof fetch;

const cache = await import('./yt-cache.ts');

// Fill the cache past its 1 GiB budget with ordinary (unpinned) plays, oldest-accessed first, then
// pin one of them — the one `evictIfOverBudget` would otherwise delete first.
const GiB = 1024 * 1024 * 1024;
const chunk = 200 * 1024 * 1024; // 200 MiB per track, 6 tracks = 1.2 GiB > the 1 GiB cap

for (let i = 0; i < 6; i++) {
	// cacheInBackground is fire-and-forget; ensureCached (also exported) is the awaited twin that
	// shares the same fetch/store path, so this both fills the cache and exercises that path too.
	await cache.ensureCached(`v${i}`, `http://x/stream?size=${chunk}`);
	// Space out lastAccessed so eviction order is deterministic (IndexedDB timestamps can collide
	// within the same millisecond on a fast loop otherwise).
	await new Promise((r) => setTimeout(r, 2));
}

// v0 is the oldest (least-recently-accessed) — pin it, then push the cache further over budget so
// eviction actually has to run and would pick v0 first if pinning didn't exempt it.
await cache.pin('v0', { title: 'Pinned Track', artist: 'Someone' });
await cache.ensureCached('v6', `http://x/stream?size=${chunk}`);

ok((await cache.getCached('v0')) !== null, 'pinned entry survives eviction even as the oldest one');

const downloaded = await cache.listDownloaded();
ok(downloaded.length === 1, 'listDownloaded reports exactly the one pinned track');
ok(downloaded[0].videoId === 'v0', 'listDownloaded reports the right track');
ok(downloaded[0].title === 'Pinned Track' && downloaded[0].artist === 'Someone', 'listDownloaded carries the metadata pin() stored');

const before = await cache.downloadedBytes();
ok(before === chunk, 'downloadedBytes reflects the one pinned track’s size');

await cache.unpin('v0');
ok((await cache.getCached('v0')) === null, 'unpin actually removes the entry (frees storage), not just clears the flag');
ok((await cache.listDownloaded()).length === 0, 'listDownloaded is empty after unpin');

console.log('ok');
