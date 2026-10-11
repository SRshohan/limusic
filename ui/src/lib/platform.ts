import { browser } from '$app/environment';

/** True inside the Tauri desktop webview, false in a plain browser tab (dev server or the PWA). */
export const isTauri = browser && '__TAURI_INTERNALS__' in window;

/** File System Access API — persistent directory handles, Chromium-based browsers only. */
export const supportsFsAccess = browser && 'showDirectoryPicker' in window;

/** Origin-private file system — used as the fallback store for the plain `<input type=file>` path. */
export const supportsOpfs =
	browser && typeof navigator !== 'undefined' && 'storage' in navigator && 'getDirectory' in navigator.storage;
