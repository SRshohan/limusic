// Ported from crates/player/src/lib.rs (`fade_due`/`start_crossfade`/`fade_volume`) so the browser
// engine fades the same way the desktop app's mpv decks did. Two differences from the Rust side:
// - No `perceptual_to_mpv`/cbrt conversion: that compensated for mpv's volume property being a cube
//   (gain = (volume/100)^3), which a Web Audio `GainNode` has no equivalent of — its `gain` value
//   already *is* linear amplitude, so the cos/sin weights are used directly.
// - No manual 50ms-step thread: `GainNode.gain.setValueCurveAtTime` plays a sampled curve natively,
//   so one call schedules the whole ramp instead of ticking it from JS.

/** Samples per second of ramp for the curve handed to `setValueCurveAtTime`. */
const CURVE_RESOLUTION_HZ = 20;

/**
 * Seconds left in the fade that should be running right now, or `null` if none is due yet.
 * `setting`/`pos`/`duration` are all seconds. Mirrors `fade_due` exactly: a fade can only be as
 * long as half the track (so a short interlude still crossfades something), and it is clamped to
 * what is actually left to play.
 */
export function fadeDue(setting: number, pos: number, duration: number): number | null {
	if (!(setting > 0 && Number.isFinite(pos) && Number.isFinite(duration) && duration > 1)) {
		return null;
	}
	const left = duration - pos;
	const fade = Math.min(setting, duration / 2);
	if (left > fade) return null;
	return Math.min(fade, Math.max(left, 0));
}

/**
 * Equal-power fade-out/fade-in curves for a `fadeSeconds`-long crossfade, each a `Float32Array` of
 * gain multipliers in [0, userGain] suitable for `GainNode.gain.setValueCurveAtTime`. `cos²+sin²=1`
 * keeps the combined loudness constant across the overlap instead of dipping in the middle.
 */
export function equalPowerCurves(
	fadeSeconds: number,
	userGain: number
): { out: Float32Array; in: Float32Array } {
	const steps = Math.max(2, Math.round(fadeSeconds * CURVE_RESOLUTION_HZ));
	const out = new Float32Array(steps);
	const fadeIn = new Float32Array(steps);
	for (let i = 0; i < steps; i++) {
		const turn = (i / (steps - 1)) * (Math.PI / 2);
		out[i] = userGain * Math.cos(turn);
		fadeIn[i] = userGain * Math.sin(turn);
	}
	return { out, in: fadeIn };
}
