<script lang="ts">
	import { page } from '$app/state';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Home01Icon, Search01Icon, LibraryIcon, UserGroup02Icon } from '@hugeicons/core-free-icons';
	import { t } from '$lib/i18n.svelte';
	import { np, ui } from '$lib/player.svelte';
	import { lt } from '$lib/lt.svelte';

	// Phone-width replacement for Sidebar's icon rail (web build only, below sm — see Sidebar.svelte
	// and +layout.svelte). Hidden while the full-screen Now Playing view is open: a tab bar under an
	// "immersive" view defeats the point of it (#mobile-friendly audit found the old rail staying
	// visible there even on desktop's narrower treatment).
	const nav = $derived([
		{ href: '/', label: t('nav.home'), icon: Home01Icon },
		{ href: '/search', label: t('nav.search'), icon: Search01Icon },
		{ href: '/library', label: t('nav.library'), icon: LibraryIcon }
	]);
	const isActive = (href: string) =>
		href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
</script>

{#if !np.open}
	<nav
		class="flex shrink-0 items-stretch border-t bg-sidebar text-sidebar-foreground sm:hidden"
		style="padding-bottom: env(safe-area-inset-bottom)"
	>
		{#each nav as n (n.href)}
			<a
				href={n.href}
				class="flex flex-1 flex-col items-center gap-0.5 py-2 text-[11px] font-medium {isActive(n.href)
					? 'text-primary'
					: 'text-sidebar-foreground/70'}"
			>
				<HugeiconsIcon icon={n.icon} class="h-5 w-5" />
				{n.label}
			</a>
		{/each}
		<button
			type="button"
			onclick={() => (ui.ltOpen = true)}
			class="flex flex-1 flex-col items-center gap-0.5 py-2 text-[11px] font-medium {lt.role !== 'none'
				? 'text-primary'
				: 'text-sidebar-foreground/70'}"
		>
			<HugeiconsIcon icon={UserGroup02Icon} class="h-5 w-5" />
			{t('nav.listen_together')}
		</button>
	</nav>
{/if}
