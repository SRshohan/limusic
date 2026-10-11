<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PlayListAddIcon, ShuffleIcon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as Tabs from '$lib/components/ui/tabs';
	import TrackFilter from '$lib/components/TrackFilter.svelte';
	import { local, playQueue, addTrackToPlaylist } from '$lib/local/store.svelte';
	import type { LocalTrack } from '$lib/local/types';

	let view = $state<'songs' | 'albums' | 'artists'>('songs');
	let query = $state('');
	let selected = $state<{ kind: 'album' | 'artist'; key: string; label: string } | null>(null);

	function fmtDuration(secs?: number) {
		if (!secs || !Number.isFinite(secs)) return '';
		const m = Math.floor(secs / 60);
		const s = Math.round(secs % 60)
			.toString()
			.padStart(2, '0');
		return `${m}:${s}`;
	}

	const albumKey = (t: LocalTrack) => t.album ?? 'Unknown album';
	const artistKey = (t: LocalTrack) => t.artist ?? 'Unknown artist';

	const q = $derived(query.trim().toLowerCase());
	const matches = (t: LocalTrack) =>
		!q ||
		t.title.toLowerCase().includes(q) ||
		(t.artist ?? '').toLowerCase().includes(q) ||
		(t.album ?? '').toLowerCase().includes(q);

	const songs = $derived(
		local.tracks.filter(
			(t) =>
				matches(t) &&
				(!selected || (selected.kind === 'album' ? albumKey(t) : artistKey(t)) === selected.key)
		)
	);

	type Group = { key: string; label: string; count: number; cover?: string };
	function groupBy(tracks: LocalTrack[], keyFn: (t: LocalTrack) => string): Group[] {
		const map = new Map<string, Group>();
		for (const t of tracks) {
			if (!matches(t)) continue;
			const key = keyFn(t);
			const g = map.get(key);
			if (g) g.count++;
			else map.set(key, { key, label: key, count: 1, cover: t.cover });
		}
		return [...map.values()].sort((a, b) => a.label.localeCompare(b.label));
	}
	const albums = $derived(groupBy(local.tracks, albumKey));
	const artists = $derived(groupBy(local.tracks, artistKey));

	function openGroup(kind: 'album' | 'artist', g: Group) {
		selected = { kind, key: g.key, label: g.label };
		view = 'songs';
	}

	function playAll(shuffle: boolean) {
		if (!songs.length) return;
		const order = shuffle
			? [...songs].map((t, i) => [Math.random(), t, i] as const).sort((a, b) => a[0] - b[0]).map((x) => x[1])
			: songs;
		void playQueue(order, 0);
	}

	let addingTo = $state<Record<string, string>>({});
	async function onAddToPlaylist(trackId: string) {
		const playlistId = addingTo[trackId];
		if (!playlistId) return;
		await addTrackToPlaylist(playlistId, trackId);
	}
</script>

<div class="flex flex-col gap-4">
	{#if local.loading}
		<p class="text-sm text-muted-foreground">Loading your library…</p>
	{:else if !local.tracks.length}
		<p class="text-sm text-muted-foreground">
			Nothing imported yet — add a folder or some files above.
		</p>
	{:else}
		<Tabs.Root bind:value={view}>
			<div class="mb-3 flex flex-wrap items-center justify-between gap-3">
				<Tabs.List>
					<Tabs.Trigger value="songs">Songs ({songs.length})</Tabs.Trigger>
					<Tabs.Trigger value="albums">Albums ({albums.length})</Tabs.Trigger>
					<Tabs.Trigger value="artists">Artists ({artists.length})</Tabs.Trigger>
				</Tabs.List>
				<TrackFilter bind:value={query} placeholder="Search your music" />
			</div>

			{#if selected}
				<div class="mb-3 flex items-center gap-2 text-sm text-muted-foreground">
					Filtered to <span class="font-medium text-foreground">{selected.label}</span>
					<button class="underline hover:text-foreground" onclick={() => (selected = null)}>
						clear
					</button>
				</div>
			{/if}

			<Tabs.Content value="songs">
				<div class="mb-3 flex gap-2">
					<Button size="sm" class="gap-2 rounded-full" disabled={!songs.length} onclick={() => playAll(false)}>
						<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" /> Play all
					</Button>
					<Button
						size="sm"
						variant="outline"
						class="gap-2 rounded-full"
						disabled={!songs.length}
						onclick={() => playAll(true)}
					>
						<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" /> Shuffle
					</Button>
				</div>
				<ul class="flex flex-col">
					{#each songs as track, i (track.id)}
						<li class="group flex items-center gap-3 rounded-lg px-2 py-2 hover:bg-accent/10">
							<button
								class="flex min-w-0 flex-1 items-center gap-3 text-left"
								onclick={() => playQueue(songs, i)}
							>
								{#if track.cover}
									<img src={track.cover} alt="" class="h-10 w-10 shrink-0 rounded object-cover" />
								{:else}
									<div class="h-10 w-10 shrink-0 rounded bg-muted"></div>
								{/if}
								<div class="min-w-0">
									<div class="truncate text-sm font-medium">{track.title}</div>
									<div class="truncate text-xs text-muted-foreground">
										{track.artist ?? 'Unknown artist'}{track.album ? ` — ${track.album}` : ''}
									</div>
								</div>
							</button>
							<span class="shrink-0 text-xs tabular-nums text-muted-foreground">
								{fmtDuration(track.duration)}
							</span>
							{#if local.playlists.length}
								<select
									class="hidden shrink-0 rounded border bg-background px-1 py-0.5 text-xs group-hover:block"
									bind:value={addingTo[track.id]}
								>
									<option value="">Add to…</option>
									{#each local.playlists as pl (pl.id)}
										<option value={pl.id}>{pl.title}</option>
									{/each}
								</select>
								<button
									class="hidden h-7 w-7 shrink-0 items-center justify-center rounded-full text-muted-foreground transition hover:bg-accent/20 hover:text-foreground group-hover:flex"
									aria-label="Add to playlist"
									onclick={() => onAddToPlaylist(track.id)}
								>
									<HugeiconsIcon icon={PlayListAddIcon} class="h-4 w-4" />
								</button>
							{/if}
						</li>
					{/each}
				</ul>
			</Tabs.Content>

			<Tabs.Content value="albums">
				<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
					{#each albums as g (g.key)}
						<button
							class="flex flex-col items-start gap-2 rounded-lg p-2 text-left hover:bg-accent/10"
							onclick={() => openGroup('album', g)}
						>
							{#if g.cover}
								<img src={g.cover} alt="" class="aspect-square w-full rounded-md object-cover" />
							{:else}
								<div class="aspect-square w-full rounded-md bg-muted"></div>
							{/if}
							<div class="w-full min-w-0">
								<div class="truncate text-sm font-medium">{g.label}</div>
								<div class="text-xs text-muted-foreground">{g.count} track{g.count === 1 ? '' : 's'}</div>
							</div>
						</button>
					{/each}
				</div>
			</Tabs.Content>

			<Tabs.Content value="artists">
				<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
					{#each artists as g (g.key)}
						<button
							class="flex flex-col items-center gap-2 rounded-lg p-2 text-center hover:bg-accent/10"
							onclick={() => openGroup('artist', g)}
						>
							{#if g.cover}
								<img src={g.cover} alt="" class="aspect-square w-20 rounded-full object-cover" />
							{:else}
								<div class="aspect-square w-20 rounded-full bg-muted"></div>
							{/if}
							<div class="w-full min-w-0">
								<div class="truncate text-sm font-medium">{g.label}</div>
								<div class="text-xs text-muted-foreground">{g.count} track{g.count === 1 ? '' : 's'}</div>
							</div>
						</button>
					{/each}
				</div>
			</Tabs.Content>
		</Tabs.Root>
	{/if}
</div>
