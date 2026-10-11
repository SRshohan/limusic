// Importing/rescanning a local-music source and resolving a track back to playable bytes. The
// browser-side counterpart of the desktop app's `local.rs` folder scan — same idea (walk a folder,
// read tags, index it), but the result lives in IndexedDB instead of SQLite because there is no
// server here for a plain browser tab to ask.
//
// TypeScript's bundled DOM lib declares FileSystemHandle/FileSystemFileHandle/
// FileSystemDirectoryHandle (the base File System Access API shapes) but not yet the
// permission-query extension (queryPermission/requestPermission) or `showDirectoryPicker` itself —
// those are still a Chromium-only incubating extension, not yet in the stable spec TS tracks.
/// <reference types="wicg-file-system-access" />
import { parseBlob } from 'music-metadata';
import { supportsFsAccess, supportsOpfs } from '$lib/platform';
import { deleteSource, deleteTracksForSource, putSource, putTracks } from './db';
import type { LocalSource, LocalTrack } from './types';

const AUDIO_EXT = new Set([
	'mp3', 'flac', 'ogg', 'oga', 'wav', 'wave', 'm4a', 'aac', 'opus', 'webm', 'weba', 'alac', 'aiff'
]);

function isAudioFile(name: string) {
	const ext = name.split('.').pop()?.toLowerCase();
	return !!ext && AUDIO_EXT.has(ext);
}

async function* walk(
	dir: FileSystemDirectoryHandle,
	prefix = ''
): AsyncGenerator<{ path: string; handle: FileSystemFileHandle }> {
	for await (const entry of dir.values()) {
		const path = prefix ? `${prefix}/${entry.name}` : entry.name;
		if (entry.kind === 'file') {
			if (isAudioFile(entry.name)) yield { path, handle: entry as FileSystemFileHandle };
		} else {
			yield* walk(entry as FileSystemDirectoryHandle, path);
		}
	}
}

function bufferToBase64(buf: Uint8Array): string {
	let binary = '';
	const chunk = 0x8000;
	for (let i = 0; i < buf.length; i += chunk) {
		binary += String.fromCharCode(...buf.subarray(i, i + chunk));
	}
	return btoa(binary);
}

async function readTags(
	file: File,
	sourceId: string,
	relPath: string,
	extra: Partial<LocalTrack>
): Promise<LocalTrack> {
	const fallbackTitle = file.name.replace(/\.[^.]+$/, '');
	const base = { id: crypto.randomUUID(), sourceId, relPath, addedAt: Date.now(), ...extra };
	try {
		const meta = await parseBlob(file, { duration: true });
		const picture = meta.common.picture?.[0];
		// Covers go in as data: URLs and IndexedDB has no hard size cap, but a multi-MB embedded
		// cover times a few hundred tracks adds up fast — skip outsized ones rather than cap the
		// whole library's storage.
		const cover =
			picture && picture.data.length < 400_000
				? `data:${picture.format};base64,${bufferToBase64(picture.data)}`
				: undefined;
		return {
			...base,
			title: meta.common.title || fallbackTitle,
			artist: meta.common.artist,
			album: meta.common.album,
			albumArtist: meta.common.albumartist,
			trackNo: meta.common.track?.no ?? undefined,
			discNo: meta.common.disk?.no ?? undefined,
			duration: meta.format.duration,
			cover
		};
	} catch {
		return { ...base, title: fallbackTitle };
	}
}

export async function scanDirectorySource(source: LocalSource, dirHandle: FileSystemDirectoryHandle) {
	const tracks: LocalTrack[] = [];
	for await (const { path, handle } of walk(dirHandle)) {
		const file = await handle.getFile();
		tracks.push(await readTags(file, source.id, path, { handle }));
	}
	await deleteTracksForSource(source.id);
	await putTracks(tracks);
}

export async function importDirectory(): Promise<LocalSource> {
	if (!supportsFsAccess) {
		throw new Error('This browser has no folder picker — use "Add files" instead.');
	}
	const dirHandle = await window.showDirectoryPicker({ mode: 'read' });
	const source: LocalSource = {
		id: crypto.randomUUID(),
		kind: 'directory',
		name: dirHandle.name,
		addedAt: Date.now(),
		dirHandle
	};
	await putSource(source);
	await scanDirectorySource(source, dirHandle);
	return source;
}

/** Re-request permission on the persisted handle (a user gesture, but no picker dialog) and
 *  re-walk. Only directory sources have anything to rescan — a plain-file import is a one-shot. */
export async function rescanSource(source: LocalSource) {
	if (source.kind !== 'directory' || !source.dirHandle) return;
	const handle = source.dirHandle;
	if ((await handle.queryPermission({ mode: 'read' })) !== 'granted') {
		const granted = await handle.requestPermission({ mode: 'read' });
		if (granted !== 'granted') throw new Error('Permission to read this folder was not granted.');
	}
	await scanDirectorySource(source, handle);
}

export async function importFiles(fileList: FileList): Promise<LocalSource> {
	const source: LocalSource = {
		id: crypto.randomUUID(),
		kind: 'files',
		name: `Imported ${new Date().toLocaleDateString()}`,
		addedAt: Date.now()
	};
	await putSource(source);
	// OPFS copy is the only way this import survives a reload without a durable handle — a plain
	// <input> gives us nothing re-requestable the way a File System Access handle does.
	const opfsRoot = supportsOpfs ? await navigator.storage.getDirectory() : null;
	const dir = opfsRoot ? await opfsRoot.getDirectoryHandle(source.id, { create: true }) : null;
	const tracks: LocalTrack[] = [];
	let i = 0;
	for (const file of Array.from(fileList)) {
		if (!isAudioFile(file.name)) continue;
		let opfsPath: string | undefined;
		if (dir) {
			opfsPath = `${i++}-${file.name}`;
			const fh = await dir.getFileHandle(opfsPath, { create: true });
			const writable = await fh.createWritable();
			await writable.write(file);
			await writable.close();
		}
		tracks.push(await readTags(file, source.id, file.name, opfsPath ? { opfsPath } : {}));
	}
	await putTracks(tracks);
	return source;
}

/** Re-resolves a track's bytes to a playable `File`. Never copies: a File-System-Access-backed
 *  track reads straight off disk each time; an OPFS-backed one reads the copy made at import. */
export async function resolveTrackFile(track: LocalTrack): Promise<File> {
	if (track.handle) {
		if ((await track.handle.queryPermission({ mode: 'read' })) !== 'granted') {
			const granted = await track.handle.requestPermission({ mode: 'read' });
			if (granted !== 'granted') throw new Error('Permission to read this file was not granted.');
		}
		return track.handle.getFile();
	}
	if (track.opfsPath) {
		const root = await navigator.storage.getDirectory();
		const dir = await root.getDirectoryHandle(track.sourceId);
		const fh = await dir.getFileHandle(track.opfsPath);
		return fh.getFile();
	}
	throw new Error('No playable file for this track.');
}

export async function removeSource(sourceId: string) {
	if (supportsOpfs) {
		try {
			const root = await navigator.storage.getDirectory();
			await root.removeEntry(sourceId, { recursive: true });
		} catch {
			// Nothing in OPFS for this source (it was a directory-handle import, not a files copy).
		}
	}
	await deleteSource(sourceId);
}
