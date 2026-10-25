<script lang="ts">
	// Offline Downloads page (mint-ui-redesign, specs/offline-downloads). Scoped to what task 5.2
	// actually built — per-track downloads from the track overflow menu — not the mockup's
	// "Downloaded playlists" section, which assumed a bulk playlist-level pin that was never built.
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { toast } from '$lib/player.svelte';
	import { t } from '$lib/i18n.svelte';

	const MAX_BYTES = 1024 * 1024 * 1024; // mirrors yt-cache.ts's own budget

	let downloads = $state<{ videoId: string; title: string; artist: string; thumbnail?: string; bytes: number }[]>([]);
	let loading = $state(true);
	let removing = $state<string | null>(null);

	onMount(load);

	async function load() {
		loading = true;
		try {
			downloads = await api.listDownloaded();
		} catch (e) {
			toast.error(String(e));
		} finally {
			loading = false;
		}
	}

	const totalBytes = $derived(downloads.reduce((sum, d) => sum + d.bytes, 0));
	const percentUsed = $derived(Math.min(100, (totalBytes / MAX_BYTES) * 100));

	function formatBytes(n: number): string {
		if (n >= 1024 * 1024 * 1024) return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
		if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
		return `${(n / 1024).toFixed(0)} KB`;
	}

	async function remove(videoId: string) {
		if (removing) return;
		removing = videoId;
		try {
			await api.removeDownload(videoId);
			downloads = downloads.filter((d) => d.videoId !== videoId);
		} catch (e) {
			toast.error(String(e));
		} finally {
			removing = null;
		}
	}
</script>

<svelte:head><title>Downloads</title></svelte:head>

<div class="mx-auto flex max-w-md flex-col gap-6 p-4 pb-32">
	<div class="flex items-center gap-3">
		<a href="/settings" aria-label={t('common.back')} class="flex h-9 w-9 items-center justify-center rounded-full bg-muted">
			←
		</a>
		<h1 class="font-heading text-lg font-bold">{t('settings.tabs.downloads')}</h1>
	</div>

	<p class="-mt-2 text-sm text-muted-foreground">
		Saved from YouTube Music for offline listening — separate from the app's own small play-cache,
		these stick around until you remove them.
	</p>

	<div class="rounded-2xl border bg-card p-4">
		<div class="flex items-center justify-between text-sm">
			<span class="font-semibold">Storage used</span>
			<span class="text-muted-foreground">{formatBytes(totalBytes)} of 1 GB</span>
		</div>
		<div class="mt-2 h-2 overflow-hidden rounded-full bg-muted">
			<div class="h-full rounded-full bg-primary" style="width:{percentUsed}%"></div>
		</div>
		<div class="mt-1.5 flex items-center justify-between text-xs text-muted-foreground">
			<span>{downloads.length} downloaded {downloads.length === 1 ? 'track' : 'tracks'}</span>
			<span>On this device only</span>
		</div>
	</div>

	{#if loading}
		<p class="text-sm text-muted-foreground">{t('common.loading')}</p>
	{:else if !downloads.length}
		<p class="text-sm text-muted-foreground">
			Nothing downloaded yet — use "{t('player.download')}" from any track's ⋯ menu.
		</p>
	{:else}
		<div class="flex flex-col divide-y rounded-2xl border bg-card">
			{#each downloads as d (d.videoId)}
				<div class="flex items-center gap-3 p-3">
					{#if d.thumbnail}
						<img src={d.thumbnail} alt="" class="h-10 w-10 shrink-0 rounded-lg object-cover" />
					{:else}
						<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground/50">
							♪
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<div class="truncate text-sm font-medium">{d.title}</div>
						<div class="truncate text-xs text-muted-foreground">{d.artist} · {formatBytes(d.bytes)}</div>
					</div>
					<button
						class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-50"
						disabled={removing === d.videoId}
						aria-label={t('player.remove_download')}
						onclick={() => remove(d.videoId)}
					>
						✕
					</button>
				</div>
			{/each}
		</div>
	{/if}
</div>
