<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PauseIcon,
		PlayIcon,
		PreviousIcon,
		NextIcon,
		VolumeHighIcon
	} from '@hugeicons/core-free-icons';
	import { current, next, playback, prev, seek, setCrossfadeSeconds, setVolume, toggle } from '$lib/local/store.svelte';

	const track = $derived(current());

	function fmt(secs: number) {
		if (!Number.isFinite(secs) || secs < 0) return '0:00';
		const m = Math.floor(secs / 60);
		const s = Math.round(secs % 60)
			.toString()
			.padStart(2, '0');
		return `${m}:${s}`;
	}

	let seeking = $state<number | null>(null);
</script>

{#if track}
	<div class="flex flex-col gap-2 rounded-xl border bg-card/60 p-3">
		{#if playback.error}
			<p class="text-xs text-destructive">{playback.error}</p>
		{/if}
		<div class="flex items-center gap-3">
			{#if track.cover}
				<img src={track.cover} alt="" class="h-12 w-12 shrink-0 rounded object-cover" />
			{:else}
				<div class="h-12 w-12 shrink-0 rounded bg-muted"></div>
			{/if}
			<div class="min-w-0 flex-1">
				<div class="truncate text-sm font-medium">{track.title}</div>
				<div class="truncate text-xs text-muted-foreground">{track.artist ?? 'Unknown artist'}</div>
			</div>
			<div class="flex shrink-0 items-center gap-1">
				<button
					class="flex h-9 w-9 items-center justify-center rounded-full hover:bg-accent/20"
					aria-label="Previous"
					disabled={playback.index <= 0}
					onclick={prev}
				>
					<HugeiconsIcon icon={PreviousIcon} class="h-5 w-5" />
				</button>
				<button
					class="flex h-10 w-10 items-center justify-center rounded-full bg-primary text-primary-foreground hover:bg-primary/80"
					aria-label={playback.playing ? 'Pause' : 'Play'}
					onclick={toggle}
				>
					<HugeiconsIcon icon={playback.playing ? PauseIcon : PlayIcon} class="h-5 w-5" />
				</button>
				<button
					class="flex h-9 w-9 items-center justify-center rounded-full hover:bg-accent/20"
					aria-label="Next"
					disabled={playback.index >= playback.queue.length - 1}
					onclick={next}
				>
					<HugeiconsIcon icon={NextIcon} class="h-5 w-5" />
				</button>
			</div>
		</div>

		<div class="flex items-center gap-2">
			<span class="w-10 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
				{fmt(seeking ?? playback.position)}
			</span>
			<input
				type="range"
				min="0"
				max={playback.duration || 0}
				step="0.1"
				value={seeking ?? playback.position}
				oninput={(e) => (seeking = Number(e.currentTarget.value))}
				onchange={(e) => {
					seek(Number(e.currentTarget.value));
					seeking = null;
				}}
				class="h-1 flex-1 accent-primary"
			/>
			<span class="w-10 shrink-0 text-xs tabular-nums text-muted-foreground">
				{fmt(playback.duration)}
			</span>
		</div>

		<div class="flex items-center gap-4 text-xs text-muted-foreground">
			<label class="flex items-center gap-2">
				<HugeiconsIcon icon={VolumeHighIcon} class="h-4 w-4" />
				<input
					type="range"
					min="0"
					max="1"
					step="0.01"
					value={playback.volume}
					oninput={(e) => setVolume(Number(e.currentTarget.value))}
					class="h-1 w-20 accent-primary"
				/>
			</label>
			<label class="flex items-center gap-2">
				Crossfade
				<input
					type="range"
					min="0"
					max="12"
					step="1"
					value={playback.crossfadeSeconds}
					oninput={(e) => setCrossfadeSeconds(Number(e.currentTarget.value))}
					class="h-1 w-20 accent-primary"
				/>
				<span class="tabular-nums">{playback.crossfadeSeconds}s</span>
			</label>
		</div>
	</div>
{/if}
