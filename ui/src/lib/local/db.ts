// Client-side library store for offline local music — the browser-side analog of the desktop
// app's `local.rs`/`local_tracks` SQLite table, but here it's the only copy (no server) so the PWA
// keeps working with zero network. IndexedDB via `idb`; see types.ts for the record shapes.
import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import type { LocalPlaylist, LocalPlaylistTrack, LocalSource, LocalTrack } from './types';

interface LocalDB extends DBSchema {
	sources: { key: string; value: LocalSource };
	tracks: { key: string; value: LocalTrack; indexes: { sourceId: string } };
	playlists: { key: string; value: LocalPlaylist };
	playlistTracks: {
		key: string;
		value: LocalPlaylistTrack;
		indexes: { playlistId: string; trackId: string };
	};
}

let dbPromise: Promise<IDBPDatabase<LocalDB>> | null = null;

function db() {
	if (!dbPromise) {
		dbPromise = openDB<LocalDB>('limusic-local', 1, {
			upgrade(d) {
				d.createObjectStore('sources', { keyPath: 'id' });
				const tracks = d.createObjectStore('tracks', { keyPath: 'id' });
				tracks.createIndex('sourceId', 'sourceId');
				d.createObjectStore('playlists', { keyPath: 'id' });
				const pt = d.createObjectStore('playlistTracks', { keyPath: 'id' });
				pt.createIndex('playlistId', 'playlistId');
				pt.createIndex('trackId', 'trackId');
			}
		});
	}
	return dbPromise;
}

export const getSources = async () => (await db()).getAll('sources');
export const putSource = async (s: LocalSource) => void (await (await db()).put('sources', s));

export async function deleteSource(id: string) {
	const d = await db();
	const tx = d.transaction(['sources', 'tracks'], 'readwrite');
	await tx.objectStore('sources').delete(id);
	const idx = tx.objectStore('tracks').index('sourceId');
	for await (const cursor of idx.iterate(id)) await cursor.delete();
	await tx.done;
}

export const getAllTracks = async () => (await db()).getAll('tracks');
export const getTracksForSource = async (sourceId: string) =>
	(await db()).getAllFromIndex('tracks', 'sourceId', sourceId);
export const getTrack = async (id: string) => (await db()).get('tracks', id);

export async function putTracks(ts: LocalTrack[]) {
	const d = await db();
	const tx = d.transaction('tracks', 'readwrite');
	await Promise.all(ts.map((t) => tx.store.put(t)));
	await tx.done;
}

export async function deleteTracksForSource(sourceId: string) {
	const d = await db();
	const tx = d.transaction('tracks', 'readwrite');
	const idx = tx.store.index('sourceId');
	for await (const cursor of idx.iterate(sourceId)) await cursor.delete();
	await tx.done;
}

export const getPlaylists = async () => (await db()).getAll('playlists');
export const putPlaylist = async (p: LocalPlaylist) => void (await (await db()).put('playlists', p));

export async function deletePlaylist(id: string) {
	const d = await db();
	const tx = d.transaction(['playlists', 'playlistTracks'], 'readwrite');
	await tx.objectStore('playlists').delete(id);
	const idx = tx.objectStore('playlistTracks').index('playlistId');
	for await (const cursor of idx.iterate(id)) await cursor.delete();
	await tx.done;
}

export const getPlaylistTracks = async (playlistId: string) =>
	(await db()).getAllFromIndex('playlistTracks', 'playlistId', playlistId);

export const addToPlaylist = async (playlistId: string, trackId: string) =>
	void (await (await db()).put('playlistTracks', {
		id: `${playlistId}:${trackId}`,
		playlistId,
		trackId,
		addedAt: Date.now()
	}));

export const removeFromPlaylist = async (playlistId: string, trackId: string) =>
	void (await (await db()).delete('playlistTracks', `${playlistId}:${trackId}`));
