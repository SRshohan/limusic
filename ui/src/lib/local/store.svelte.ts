// Reactive glue between the IndexedDB library (db.ts/import.ts) and the browser playback engine —
// the offline counterpart of player.svelte.ts, but self-contained: nothing here ever calls the
// server, by design (see the Phase 1 plan — this is what keeps local music playable with zero
// network at all, not just "no YouTube").
import { browser } from '$app/environment';
import { AudioEngine } from '$lib/engine/audio-engine';
import { bindMediaSession, setNowPlayingMeta } from '$lib/engine/mediasession';
import * as db from './db';
import * as importer from './import';
import type { LocalPlaylist, LocalSource, LocalTrack } from './types';

export const local = $state({
	sources: [] as LocalSource[],
	tracks: [] as LocalTrack[],
	playlists: [] as LocalPlaylist[],
	loading: false,
	error: null as string | null
});

export const playback = $state({
	queue: [] as LocalTrack[],
	index: -1,
	playing: false,
	position: 0,
	duration: 0,
	volume: 1,
	crossfadeSeconds: 0,
	error: null as string | null
});

export const current = () => playback.queue[playback.index] as LocalTrack | undefined;

let engine: AudioEngine | null = null;

function getEngine() {
	if (!engine) {
		engine = new AudioEngine();
		engine.addEventListener('position', (e) => {
			playback.position = (e as CustomEvent).detail.seconds;
		});
		engine.addEventListener('duration', (e) => {
			playback.duration = (e as CustomEvent).detail.seconds;
		});
		engine.addEventListener('playstate', (e) => {
			playback.playing = (e as CustomEvent).detail.playing;
		});
		engine.addEventListener('advanced', () => void onAdvanced());
		engine.addEventListener('ended', () => void onEndedWithoutPreload());
		engine.addEventListener('error', (e) => {
			playback.error = (e as CustomEvent).detail.message;
		});
		bindMediaSession(engine, { next, prev });
	}
	return engine;
}

export async function loadLibrary() {
	local.loading = true;
	try {
		[local.sources, local.tracks, local.playlists] = await Promise.all([
			db.getSources(),
			db.getAllTracks(),
			db.getPlaylists()
		]);
	} finally {
		local.loading = false;
	}
}

async function refresh() {
	[local.sources, local.tracks] = await Promise.all([db.getSources(), db.getAllTracks()]);
}

export async function createPlaylist(title: string) {
	const now = Date.now();
	await db.putPlaylist({ id: crypto.randomUUID(), title, createdAt: now, updatedAt: now });
	local.playlists = await db.getPlaylists();
}

export async function renamePlaylist(playlist: LocalPlaylist, title: string) {
	await db.putPlaylist({ ...playlist, title, updatedAt: Date.now() });
	local.playlists = await db.getPlaylists();
}

export async function deletePlaylistById(id: string) {
	await db.deletePlaylist(id);
	local.playlists = await db.getPlaylists();
}

export const getPlaylistTrackIds = async (playlistId: string) =>
	(await db.getPlaylistTracks(playlistId)).map((pt) => pt.trackId);

export async function addTrackToPlaylist(playlistId: string, trackId: string) {
	await db.addToPlaylist(playlistId, trackId);
}

export async function removeTrackFromPlaylist(playlistId: string, trackId: string) {
	await db.removeFromPlaylist(playlistId, trackId);
}

export async function addFolder() {
	local.error = null;
	try {
		await importer.importDirectory();
		await refresh();
	} catch (e) {
		local.error = e instanceof Error ? e.message : String(e);
	}
}

export async function addFiles(fileList: FileList) {
	local.error = null;
	try {
		await importer.importFiles(fileList);
		await refresh();
	} catch (e) {
		local.error = e instanceof Error ? e.message : String(e);
	}
}

export async function rescan(source: LocalSource) {
	local.error = null;
	try {
		await importer.rescanSource(source);
		await refresh();
	} catch (e) {
		local.error = e instanceof Error ? e.message : String(e);
	}
}

export async function removeSource(id: string) {
	await importer.removeSource(id);
	await refresh();
}

async function urlForTrack(track: LocalTrack) {
	const file = await importer.resolveTrackFile(track);
	return URL.createObjectURL(file);
}

function announceNowPlaying(track: LocalTrack) {
	setNowPlayingMeta({ title: track.title, artist: track.artist, album: track.album, artwork: track.cover });
}

export async function playQueue(tracks: LocalTrack[], startIndex: number) {
	playback.queue = tracks;
	playback.index = startIndex;
	await playCurrent(true);
}

async function playCurrent(autoplay: boolean) {
	const track = current();
	if (!track) return;
	playback.error = null;
	try {
		const url = await urlForTrack(track);
		await getEngine().loadTrack(url, { autoplay, objectUrl: true });
		announceNowPlaying(track);
		void preloadNext();
	} catch (e) {
		playback.error = e instanceof Error ? e.message : String(e);
	}
}

async function preloadNext() {
	const next = playback.queue[playback.index + 1];
	if (!next) return;
	try {
		const url = await urlForTrack(next);
		getEngine().preload(url, { objectUrl: true });
	} catch {
		// Couldn't resolve the next file ahead of time (permission lapsed, file moved) — it falls
		// back to a normal full load in `onEndedWithoutPreload` once playback actually gets there.
	}
}

async function onAdvanced() {
	if (playback.index + 1 >= playback.queue.length) return;
	playback.index += 1;
	const track = current();
	if (track) announceNowPlaying(track);
	void preloadNext();
}

async function onEndedWithoutPreload() {
	if (playback.index + 1 >= playback.queue.length) return;
	playback.index += 1;
	await playCurrent(true);
}

export function next() {
	if (playback.index + 1 < playback.queue.length) {
		playback.index += 1;
		void playCurrent(true);
	}
}
export function prev() {
	if (playback.index > 0) {
		playback.index -= 1;
		void playCurrent(true);
	}
}
export function toggle() {
	getEngine().toggle();
}
export function seek(seconds: number) {
	getEngine().seek(seconds);
}
export function setVolume(v: number) {
	playback.volume = v;
	getEngine().setVolume(v);
}
export function setCrossfadeSeconds(s: number) {
	playback.crossfadeSeconds = s;
	getEngine().setCrossfadeSeconds(s);
}

if (browser) void loadLibrary();
