// Lock-screen/OS media-key integration. Replaces `souvlaki`/media.rs (MPRIS/SMTC) with far less
// code: a real browser already forwards hardware/OS media-key presses to the page that owns the
// active `MediaSession`, so there is nothing here to poll or bind to a native API.
import type { AudioEngine } from './audio-engine';

export type NowPlayingMeta = {
	title: string;
	artist?: string;
	album?: string;
	artwork?: string;
};

export function bindMediaSession(
	engine: AudioEngine,
	handlers: { next?: () => void; prev?: () => void }
) {
	if (!('mediaSession' in navigator)) return () => {};
	const ms = navigator.mediaSession;
	ms.setActionHandler('play', () => engine.play());
	ms.setActionHandler('pause', () => engine.pause());
	ms.setActionHandler('seekto', (d) => {
		if (typeof d.seekTime === 'number') engine.seek(d.seekTime);
	});
	ms.setActionHandler('previoustrack', handlers.prev ?? null);
	ms.setActionHandler('nexttrack', handlers.next ?? null);

	const onPlaystate = (e: Event) => {
		ms.playbackState = (e as CustomEvent).detail.playing ? 'playing' : 'paused';
	};
	const onDuration = () => updatePositionState();
	const onPosition = () => updatePositionState();
	const updatePositionState = () => {
		const duration = engine.duration;
		if (!Number.isFinite(duration) || duration <= 0) return;
		try {
			ms.setPositionState({ duration, position: Math.min(engine.position, duration), playbackRate: 1 });
		} catch {
			// Thrown if position > duration during a transient buffering moment — not worth surfacing.
		}
	};
	engine.addEventListener('playstate', onPlaystate);
	engine.addEventListener('duration', onDuration);
	engine.addEventListener('position', onPosition);

	return () => {
		engine.removeEventListener('playstate', onPlaystate);
		engine.removeEventListener('duration', onDuration);
		engine.removeEventListener('position', onPosition);
		ms.setActionHandler('play', null);
		ms.setActionHandler('pause', null);
		ms.setActionHandler('seekto', null);
		ms.setActionHandler('previoustrack', null);
		ms.setActionHandler('nexttrack', null);
	};
}

export function setNowPlayingMeta(meta: NowPlayingMeta) {
	if (!('mediaSession' in navigator)) return;
	navigator.mediaSession.metadata = new MediaMetadata({
		title: meta.title,
		artist: meta.artist ?? '',
		album: meta.album ?? '',
		artwork: meta.artwork ? [{ src: meta.artwork, sizes: '512x512', type: 'image/png' }] : []
	});
}
