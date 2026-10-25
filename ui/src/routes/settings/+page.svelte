<script lang="ts">
	// Mobile-native Settings page (mint-ui-redesign) — a grouped list, not a port of the desktop
	// `SettingsDialog.svelte` tabbed dialog, which stays desktop-only. Scoped to the rows the
	// mockup (`settings.html`) showed: toggles are fully wired to real state; the navigational rows
	// (quality/crossfade/lyrics providers) show the real current value but don't grow a new picker
	// UI here — that's bigger than this task asked for and the mockup didn't show one either.
	import { onMount } from 'svelte';
	import { mode, toggleMode } from 'mode-watcher';
	import * as api from '$lib/api';
	import { auth, toast } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import { t } from '$lib/i18n.svelte';

	let settings = $state<Record<string, string>>({});
	let lastfm = $state<{ connected: boolean; username?: string | null } | null>(null);
	let discordBusy = $state(false);

	onMount(async () => {
		try {
			settings = await api.getSettings();
		} catch (e) {
			toast.error(String(e));
		}
		try {
			lastfm = await api.lastfmStatus();
		} catch {
			// Last.fm status is a nice-to-have on this row; a failure here just leaves it unconnected.
		}
	});

	const quality = $derived(settings.quality ?? 'HIGH');
	const crossfadeSecs = $derived(settings.crossfade_secs ?? '5');
	const discordOn = $derived(settings.discord_rpc === 'true');

	async function toggleDiscord() {
		if (discordBusy) return;
		discordBusy = true;
		const next = !discordOn;
		const prev = settings.discord_rpc;
		settings.discord_rpc = next ? 'true' : 'false';
		try {
			await api.setSetting('discord_rpc', settings.discord_rpc);
		} catch (e) {
			settings.discord_rpc = prev;
			toast.error(String(e));
		} finally {
			discordBusy = false;
		}
	}
</script>

<svelte:head><title>{t('nav.settings')}</title></svelte:head>

<div class="mx-auto flex max-w-md flex-col gap-6 p-4 pb-32">
	<div class="flex items-center gap-3">
		<a href="/profile" aria-label={t('common.back')} class="flex h-9 w-9 items-center justify-center rounded-full bg-muted">
			←
		</a>
		<h1 class="font-heading text-lg font-bold">{t('nav.settings')}</h1>
	</div>

	<section>
		<h2 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('nav.account')}</h2>
		<a href="/profile" class="flex items-center gap-3 rounded-2xl border bg-card p-3">
			{#if auth.account?.thumbnail}
				<img src={thumb(auth.account.thumbnail, 64)} alt="" class="h-8 w-8 shrink-0 rounded-full object-cover" />
			{:else}
				<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-sm font-bold text-primary-foreground">
					{(auth.account?.name ?? '?').slice(0, 1).toUpperCase()}
				</div>
			{/if}
			<div class="min-w-0 flex-1">
				<div class="truncate text-sm font-medium">{auth.account?.name ?? t('nav.account')}</div>
				<div class="truncate text-xs text-muted-foreground">{t('nav.saved_accounts_hint')}</div>
			</div>
			<span class="text-muted-foreground">›</span>
		</a>
	</section>

	<section>
		<h2 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('settings.tabs.themes')}</h2>
		<div class="flex flex-col divide-y rounded-2xl border bg-card">
			<div class="flex items-center gap-3 p-3">
				<span class="flex-1 text-sm font-medium">{t('settings.themes.dark_mode')}</span>
				<button
					role="switch"
					aria-checked={mode.current === 'dark'}
					aria-label={t('settings.themes.dark_mode')}
					onclick={toggleMode}
					class="relative h-6 w-10 rounded-full transition-colors {mode.current === 'dark' ? 'bg-primary' : 'bg-muted'}"
				>
					<span
						class="absolute top-0.5 h-5 w-5 rounded-full bg-background shadow transition-transform {mode.current === 'dark' ? 'translate-x-[18px]' : 'translate-x-0.5'}"
					></span>
				</button>
			</div>
		</div>
	</section>

	<section>
		<h2 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('settings.tabs.playback')}</h2>
		<div class="flex flex-col divide-y rounded-2xl border bg-card">
			<div class="flex items-center gap-3 p-3">
				<div class="flex-1">
					<div class="text-sm font-medium">{t('settings.playback.audio_quality')}</div>
				</div>
				<span class="text-sm text-muted-foreground">{quality}</span>
			</div>
			<div class="flex items-center gap-3 p-3">
				<div class="flex-1">
					<div class="text-sm font-medium">{t('settings.playback.crossfade')}</div>
				</div>
				<span class="text-sm text-muted-foreground">{crossfadeSecs}s</span>
			</div>
		</div>
	</section>

	<section>
		<h2 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('settings.tabs.general')}</h2>
		<div class="flex flex-col divide-y rounded-2xl border bg-card">
			<div class="flex items-center gap-3 p-3">
				<span class="flex-1 text-sm font-medium">{t('settings.general.discord_rpc')}</span>
				<button
					role="switch"
					aria-checked={discordOn}
					aria-label={t('settings.general.discord_rpc')}
					disabled={discordBusy}
					onclick={toggleDiscord}
					class="relative h-6 w-10 rounded-full transition-colors disabled:opacity-50 {discordOn ? 'bg-primary' : 'bg-muted'}"
				>
					<span
						class="absolute top-0.5 h-5 w-5 rounded-full bg-background shadow transition-transform {discordOn ? 'translate-x-[18px]' : 'translate-x-0.5'}"
					></span>
				</button>
			</div>
			<div class="flex items-center gap-3 p-3">
				<div class="flex-1 text-sm font-medium">Last.fm</div>
				<span class="text-sm text-muted-foreground">
					{lastfm?.connected ? (lastfm.username ?? '✓') : '—'}
				</span>
			</div>
		</div>
	</section>

	<section>
		<h2 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('settings.tabs.offline')}</h2>
		<div class="flex flex-col divide-y rounded-2xl border bg-card">
			<a href="/downloads" class="flex items-center gap-3 p-3">
				<span class="flex-1 text-sm font-medium">{t('settings.tabs.downloads')}</span>
				<span class="text-muted-foreground">›</span>
			</a>
			<a href="/offline" class="flex items-center gap-3 p-3">
				<span class="flex-1 text-sm font-medium">{t('settings.tabs.local_files_tab')}</span>
				<span class="text-muted-foreground">›</span>
			</a>
		</div>
	</section>

	<section>
		<h2 class="mb-2 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{t('settings.tabs.about')}</h2>
		<div class="flex flex-col items-center gap-2 rounded-2xl border bg-card p-4 text-center">
			<img src="/icons/icon.svg" alt="" width="48" height="48" class="rounded-[22%]" />
			<div class="text-sm font-bold">SRMusic</div>
		</div>
	</section>
</div>
