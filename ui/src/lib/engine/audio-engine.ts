// Browser-native replacement for the desktop app's libmpv-backed `Player` (crates/player). Two
// `<audio>` "decks" through a Web Audio graph stand in for mpv's two-deck gapless/crossfade setup;
// see crossfade.ts for the ported fade math. Deliberately source-agnostic: `loadTrack`/`preload`
// take any URL a browser can fetch — a local `blob:` URL today (Phase 1), a `/media/stream/:id`
// server URL later (Phase 4) — so this engine is not rewritten when the YouTube path lands.

import { fadeDue, equalPowerCurves } from './crossfade';

export type EngineEventMap = {
	position: { seconds: number };
	duration: { seconds: number };
	playstate: { playing: boolean };
	/** The deck swapped to a preloaded successor (crossfade start, or a gapless end-of-track). The
	 *  caller should advance its queue pointer here, same as the desktop app's `TrackEnded`. */
	advanced: object;
	/** This deck's media ended with no preloaded successor — nothing to swap to. */
	ended: object;
	error: { message: string };
};

type Deck = {
	audio: HTMLAudioElement;
	source: MediaElementAudioSourceNode;
	gain: GainNode;
	preloadedUrl: string | null;
	objectUrl: string | null; // revoked on replacement/destroy if we created it
};

const dbToGain = (db: number) => Math.pow(10, db / 20);

export class AudioEngine extends EventTarget {
	#ctx: AudioContext;
	#decks: [Deck, Deck];
	#active = 0;
	#masterGain = 1;
	#crossfadeSeconds = 0;
	#fading = false;
	#fadeToken = 0;

	constructor() {
		super();
		this.#ctx = new AudioContext();
		this.#decks = [this.#buildDeck(), this.#buildDeck()];
	}

	#buildDeck(): Deck {
		const audio = new Audio();
		audio.preload = 'auto';
		audio.crossOrigin = 'anonymous';
		const source = this.#ctx.createMediaElementSource(audio);
		const gain = this.#ctx.createGain();
		gain.gain.value = 0;
		source.connect(gain).connect(this.#ctx.destination);
		const deck: Deck = { audio, source, gain, preloadedUrl: null, objectUrl: null };
		audio.addEventListener('timeupdate', () => this.#onTimeUpdate(deck));
		audio.addEventListener('durationchange', () => {
			if (deck === this.#activeDeck() && Number.isFinite(audio.duration)) {
				this.dispatchEvent(new CustomEvent('duration', { detail: { seconds: audio.duration } }));
			}
		});
		audio.addEventListener('play', () => {
			if (deck === this.#activeDeck()) {
				this.dispatchEvent(new CustomEvent('playstate', { detail: { playing: true } }));
			}
		});
		audio.addEventListener('pause', () => {
			if (deck === this.#activeDeck() && !this.#fading) {
				this.dispatchEvent(new CustomEvent('playstate', { detail: { playing: false } }));
			}
		});
		audio.addEventListener('ended', () => this.#onEnded(deck));
		audio.addEventListener('error', () => {
			if (deck === this.#activeDeck()) {
				const message = audio.error?.message || 'playback error';
				this.dispatchEvent(new CustomEvent('error', { detail: { message } }));
			}
		});
		return deck;
	}

	#activeDeck() {
		return this.#decks[this.#active];
	}
	#idleDeck() {
		return this.#decks[1 - this.#active];
	}

	#onTimeUpdate(deck: Deck) {
		if (deck !== this.#activeDeck()) return;
		this.dispatchEvent(
			new CustomEvent('position', { detail: { seconds: deck.audio.currentTime } })
		);
		if (this.#fading || !this.#idleDeck().preloadedUrl) return;
		const fade = fadeDue(this.#crossfadeSeconds, deck.audio.currentTime, deck.audio.duration);
		if (fade !== null) this.#startCrossfade(fade);
	}

	#onEnded(deck: Deck) {
		if (deck !== this.#activeDeck() || this.#fading) return;
		const idle = this.#idleDeck();
		if (idle.preloadedUrl) {
			// Gapless (crossfade off, or a fade that never got the chance to run): hard swap now.
			this.#swapToIdle();
			idle.audio.play().catch(() => {});
			this.dispatchEvent(new CustomEvent('advanced'));
		} else {
			this.dispatchEvent(new CustomEvent('ended'));
		}
	}

	#swapToIdle() {
		this.#active = 1 - this.#active;
	}

	#startCrossfade(fadeSeconds: number) {
		const out = this.#activeDeck();
		const incoming = this.#idleDeck();
		this.#fading = true;
		const token = ++this.#fadeToken;
		const { out: outCurve, in: inCurve } = equalPowerCurves(fadeSeconds, this.#masterGain);
		const now = this.#ctx.currentTime;
		out.gain.gain.cancelScheduledValues(now);
		incoming.gain.gain.cancelScheduledValues(now);
		out.gain.gain.setValueCurveAtTime(outCurve, now, fadeSeconds);
		incoming.gain.gain.setValueCurveAtTime(inCurve, now, fadeSeconds);
		incoming.audio.play().catch(() => {});
		this.#swapToIdle();
		this.dispatchEvent(new CustomEvent('advanced'));
		setTimeout(() => {
			if (token !== this.#fadeToken) return; // a seek/skip/pause cancelled this fade already
			out.audio.pause();
			out.audio.currentTime = 0;
			out.gain.gain.cancelScheduledValues(this.#ctx.currentTime);
			out.gain.gain.value = 0;
			this.#releaseDeckUrl(out);
			out.preloadedUrl = null;
			this.#fading = false;
		}, fadeSeconds * 1000);
	}

	#releaseDeckUrl(deck: Deck) {
		if (deck.objectUrl) URL.revokeObjectURL(deck.objectUrl);
		deck.objectUrl = null;
	}

	/** Cancels any in-flight fade immediately, bringing the active deck to full volume — mirrors
	 *  the desktop app's "a seek/pause/skip cancels the ramp" rule (`finish_fade`, cancelled path). */
	#cancelFade() {
		if (!this.#fading) return;
		this.#fadeToken++;
		this.#fading = false;
		const active = this.#activeDeck();
		active.gain.gain.cancelScheduledValues(this.#ctx.currentTime);
		active.gain.gain.value = this.#masterGain;
		const idle = this.#idleDeck();
		idle.audio.pause();
		idle.gain.gain.cancelScheduledValues(this.#ctx.currentTime);
		idle.gain.gain.value = 0;
	}

	/** Loads `url` onto the active deck, replacing whatever is playing, and optionally starts it. */
	async loadTrack(
		url: string,
		opts: { gainDb?: number; autoplay?: boolean; objectUrl?: boolean } = {}
	) {
		this.#cancelFade();
		await this.#ctx.resume();
		const idle = this.#idleDeck();
		idle.audio.pause();
		this.#releaseDeckUrl(idle);
		idle.preloadedUrl = null;
		const deck = this.#activeDeck();
		this.#releaseDeckUrl(deck);
		deck.objectUrl = opts.objectUrl ? url : null;
		deck.preloadedUrl = null;
		deck.audio.src = url;
		deck.audio.currentTime = 0;
		deck.gain.gain.cancelScheduledValues(this.#ctx.currentTime);
		deck.gain.gain.value = this.#masterGain * dbToGain(opts.gainDb ?? 0);
		if (opts.autoplay) await deck.audio.play().catch(() => {});
	}

	/** Loads `url` onto the idle deck for a gapless/crossfade handoff when the active deck ends. */
	preload(url: string, opts: { gainDb?: number; objectUrl?: boolean } = {}) {
		const idle = this.#idleDeck();
		this.#releaseDeckUrl(idle);
		idle.objectUrl = opts.objectUrl ? url : null;
		idle.audio.src = url;
		idle.audio.currentTime = 0;
		idle.gain.gain.cancelScheduledValues(this.#ctx.currentTime);
		idle.gain.gain.value = this.#masterGain * dbToGain(opts.gainDb ?? 0);
		idle.preloadedUrl = url;
	}

	/** Drops any preloaded successor without playing it (the queue changed before it was needed). */
	clearPreload() {
		const idle = this.#idleDeck();
		this.#releaseDeckUrl(idle);
		idle.audio.removeAttribute('src');
		idle.audio.load();
		idle.preloadedUrl = null;
	}

	/** Call synchronously inside a user-gesture handler (a click), before any `await` — a browser's
	 *  autoplay policy only counts an `AudioContext.resume()` as gesture-triggered if it happens
	 *  within that same synchronous call stack. An `await` on a network fetch (resolving a stream
	 *  URL, say) before the first `resume()`/`play()` call falls outside that window, and the
	 *  context is then silently stuck "suspended": the underlying `<audio>` element still decodes
	 *  and advances its own `currentTime` normally (so position/duration/pause-state all look
	 *  right), but nothing reaches the speakers, because the Web Audio graph it's routed through
	 *  (source → gain → destination) never started. */
	primeForGesture() {
		void this.#ctx.resume();
	}

	async play() {
		await this.#ctx.resume();
		await this.#activeDeck().audio.play().catch(() => {});
	}
	pause() {
		this.#cancelFade();
		this.#activeDeck().audio.pause();
	}
	toggle() {
		this.#activeDeck().audio.paused ? this.play() : this.pause();
	}
	seek(seconds: number) {
		this.#cancelFade();
		this.#activeDeck().audio.currentTime = seconds;
	}

	get position() {
		return this.#activeDeck().audio.currentTime;
	}
	get duration() {
		return this.#activeDeck().audio.duration;
	}
	get paused() {
		return this.#activeDeck().audio.paused;
	}

	setVolume(v: number) {
		this.#masterGain = Math.max(0, Math.min(1, v));
		if (!this.#fading) this.#activeDeck().gain.gain.value = this.#masterGain;
	}
	setCrossfadeSeconds(n: number) {
		this.#crossfadeSeconds = Math.max(0, n);
	}

	destroy() {
		this.#cancelFade();
		for (const deck of this.#decks) {
			deck.audio.pause();
			deck.audio.removeAttribute('src');
			this.#releaseDeckUrl(deck);
		}
		this.#ctx.close().catch(() => {});
	}
}
