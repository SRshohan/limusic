<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Add01Icon, Delete02Icon, PlayIcon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import {
		createPlaylist,
		deletePlaylistById,
		getPlaylistTrackIds,
		local,
		playQueue,
		removeTrackFromPlaylist
	} from '$lib/local/store.svelte';
	import type { LocalTrack } from '$lib/local/types';

	let newTitle = $state('');
	let expanded = $state<string | null>(null);
	let expandedTracks = $state<LocalTrack[]>([]);

	async function onCreate() {
		const title = newTitle.trim();
		if (!title) return;
		await createPlaylist(title);
		newTitle = '';
	}

	async function toggleExpand(playlistId: string) {
		if (expanded === playlistId) {
			expanded = null;
			return;
		}
		expanded = playlistId;
		const ids = await getPlaylistTrackIds(playlistId);
		expandedTracks = ids
			.map((id) => local.tracks.find((t) => t.id === id))
			.filter((t): t is LocalTrack => !!t);
	}

	async function onRemoveTrack(playlistId: string, trackId: string) {
		await removeTrackFromPlaylist(playlistId, trackId);
		expandedTracks = expandedTracks.filter((t) => t.id !== trackId);
	}
</script>

<div class="rounded-xl border bg-card/40 p-4">
	<div class="mb-3 flex items-center gap-2">
		<input
			bind:value={newTitle}
			placeholder="New playlist name"
			class="min-w-0 flex-1 rounded-lg border bg-background px-3 py-1.5 text-sm outline-none focus:border-accent"
			onkeydown={(e) => e.key === 'Enter' && onCreate()}
		/>
		<Button size="sm" class="gap-2" disabled={!newTitle.trim()} onclick={onCreate}>
			<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /> Create
		</Button>
	</div>

	{#if !local.playlists.length}
		<p class="text-sm text-muted-foreground">No playlists yet.</p>
	{:else}
		<ul class="flex flex-col gap-1">
			{#each local.playlists as pl (pl.id)}
				<li class="rounded-lg">
					<div class="flex items-center justify-between gap-3 px-2 py-1.5 hover:bg-accent/10">
						<button class="min-w-0 flex-1 truncate text-left text-sm font-medium" onclick={() => toggleExpand(pl.id)}>
							{pl.title}
						</button>
						<button
							class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-muted-foreground transition hover:bg-destructive/10 hover:text-destructive"
							aria-label="Delete playlist"
							onclick={() => deletePlaylistById(pl.id)}
						>
							<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" />
						</button>
					</div>
					{#if expanded === pl.id}
						<div class="ml-2 flex flex-col border-l pl-3">
							{#if !expandedTracks.length}
								<p class="py-1 text-xs text-muted-foreground">No tracks yet — add some from the library.</p>
							{:else}
								<button
									class="my-1 flex w-fit items-center gap-1.5 rounded-full bg-primary/10 px-2.5 py-1 text-xs text-primary hover:bg-primary/20"
									onclick={() => playQueue(expandedTracks, 0)}
								>
									<HugeiconsIcon icon={PlayIcon} class="h-3.5 w-3.5" /> Play all
								</button>
								{#each expandedTracks as t (t.id)}
									<div class="flex items-center justify-between gap-2 py-1 text-sm">
										<span class="min-w-0 truncate">{t.title}</span>
										<button
											class="shrink-0 text-xs text-muted-foreground hover:text-destructive"
											onclick={() => onRemoveTrack(pl.id, t.id)}
										>
											Remove
										</button>
									</div>
								{/each}
							{/if}
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</div>
