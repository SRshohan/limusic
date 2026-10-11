export type LocalSourceKind = 'directory' | 'files';

export interface LocalSource {
	id: string;
	kind: LocalSourceKind;
	name: string;
	addedAt: number;
	/** Persisted so a rescan can re-request permission instead of re-opening the folder picker. */
	dirHandle?: FileSystemDirectoryHandle;
}

export interface LocalTrack {
	id: string;
	sourceId: string;
	/** Relative path under the source, used for display and as the dedupe key on rescan. */
	relPath: string;
	title: string;
	artist?: string;
	album?: string;
	albumArtist?: string;
	trackNo?: number;
	discNo?: number;
	duration?: number;
	/** Small data: URL extracted from embedded tags, if any. */
	cover?: string;
	addedAt: number;
	/** File System Access API path — re-resolved to a `File` each session, nothing duplicated. */
	handle?: FileSystemFileHandle;
	/** Plain-`<input>` fallback path: the bytes live in OPFS at this path (no durable handle). */
	opfsPath?: string;
}

export interface LocalPlaylist {
	id: string;
	title: string;
	createdAt: number;
	updatedAt: number;
}

export interface LocalPlaylistTrack {
	id: string;
	playlistId: string;
	trackId: string;
	addedAt: number;
}
