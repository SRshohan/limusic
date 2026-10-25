<script lang="ts">
	// A horizontal shelf, drawn according to what it actually holds.
	//
	// Every shelf is the same big-art card rail — artists and playlists get forms shaped for them
	// (a poster frame, a cover with the stack behind it), everything else, songs included, is the
	// plain card, so a shelf never reads as a wall of unrelated layouts.
	//
	// The rail, its arrows, the edge fades and the content-visibility budget are shared by all of
	// them; only the slot changes.
	import { HugeiconsIcon, type IconSvgElement } from '@hugeicons/svelte';
	import {
		ArrowLeft01Icon,
		ArrowRight01Icon,
		CdIcon,
		PlayIcon,
		PlayListIcon,
		UserMultiple02Icon
	} from '@hugeicons/core-free-icons';
	import MediaCard from './MediaCard.svelte';
	import CommunityCard from './CommunityCard.svelte';
	import PortraitCard from './PortraitCard.svelte';
	import StackCard from './StackCard.svelte';
	import SectionHeading from './SectionHeading.svelte';
	import * as api from '$lib/api';
	import type { BrowseItem } from '$lib/api';
	import type { CardSize } from '$lib/personal';
	import { asSong } from '$lib/browse';
	import { openPlayer } from '$lib/player.svelte';
	import { t } from '$lib/i18n.svelte';

	let {
		title,
		items,
		onMore,
		community = false,
		rich = true,
		size = 'medium',
		icon
	}: {
		title?: string;
		items: BrowseItem[];
		/** Renders a "See all" button in the header when provided. */
		onMore?: () => void;
		/**
		 * Community playlist cards: four per row at most, stretching to fill the width instead of
		 * fitting more cards as the window grows. The arrows page through the rest.
		 */
		community?: boolean;
		/** Opt out of the per-kind forms and render plain cards. */
		rich?: boolean;
		/** Card width, set in Edit home. Community cards keep their own. */
		size?: CardSize;
		/** Overrides the heading icon this shelf's dominant kind would otherwise pick. */
		icon?: IconSvgElement;
	} = $props();

	// A shelf is only worth a form of its own when it's overwhelmingly one kind of thing. Below the
	// threshold it's a mixed bag ("Listen again"), and the plain card is the honest way to draw it.
	const MOSTLY = 0.75;
	type Mode = 'album' | 'artist' | 'playlist' | 'card';
	const mode = $derived.by<Mode>(() => {
		if (community || !rich || !items.length) return 'card';
		const counts = new Map<string, number>();
		for (const i of items) counts.set(i.kind, (counts.get(i.kind) ?? 0) + 1);
		const [kind, n] = [...counts].sort((a, b) => b[1] - a[1])[0];
		if (kind === 'song') return 'card'; // songs get the plain card too, same as everything else
		return n / items.length >= MOSTLY ? (kind as Mode) : 'card';
	});

	const ICONS: Record<Mode, IconSvgElement | undefined> = {
		album: CdIcon,
		artist: UserMultiple02Icon,
		playlist: PlayListIcon,
		card: undefined
	};

	// A shelf that's mostly songs still gets a "Play all" in its header — queuing the set is still
	// useful even though each song now sits in its own card rather than a numbered row.
	const songs = $derived(items.filter((i) => i.kind === 'song').map(asSong));
	const mostlySongs = $derived(items.length > 0 && songs.length / items.length >= MOSTLY);
	// The header's Play button queues the whole shelf.
	const playAll = () => {
		openPlayer();
		return api.playPlaylist(songs, 0, undefined, title);
	};

	// Slot width per form, and the height the rail reserves before it has been laid out: a card of
	// `size` width, a playlist's a rem wider for the stack showing behind it. Inline widths, not w-*
	// classes: a stale dev stylesheet that hasn't generated one collapses the card to its artwork's
	// size.
	const WIDTH: Record<CardSize, number> = { small: 8, medium: 10, large: 13 };
	const cardWidth = (playlist = false) => `width:${WIDTH[size] + (playlist ? 1 : 0)}rem`;
	// A card is about its width plus 7.5rem of heading, caption and padding.
	const height = `${WIDTH[size] + 7.5}rem`;

	let row = $state<HTMLDivElement | null>(null);
	let canLeft = $state(false);
	let canRight = $state(false);

	function update() {
		if (!row) return;
		canLeft = row.scrollLeft > 4;
		canRight = row.scrollLeft + row.clientWidth < row.scrollWidth - 4;
	}

	let hovered = false;
	const measureOnEnter = (el: HTMLElement) => {
		const enter = () => {
			hovered = true;
			update();
		};
		const leave = () => (hovered = false);
		el.addEventListener('pointerenter', enter);
		el.addEventListener('pointerleave', leave);
		return () => {
			el.removeEventListener('pointerenter', enter);
			el.removeEventListener('pointerleave', leave);
		};
	};

	function page(dir: 1 | -1) {
		row?.scrollBy({ left: dir * Math.round(row.clientWidth * 0.9), behavior: 'smooth' });
	}

	// Re-measure when the content changes under the pointer, and only then. Measuring at mount reads
	// scrollWidth before content-visibility has skipped anything, which makes WebKitGTK lay out every
	// shelf on the page synchronously: going back to Home spent ~640 of its ~870 ms there with 8
	// shelves (perf/navprobe.py). The arrows only show on hover, and pointerenter measures then.
	$effect(() => {
		items;
		if (hovered) update();
	});
</script>

<svelte:window onresize={update} />

<!-- content-visibility: a home feed grows to hundreds of cards and WebKit keeps every one of them in
     style, layout and paint. Because it rasterizes in tiles, one card's hover repaint re-rasterizes
     the images around it, so hovering gets slower the further you scroll. Skipping off-screen
     shelves caps that at a screenful. The intrinsic size is a shelf of this form (heading + row);
     the `auto` keyword swaps in the real size once measured, so the scrollbar stays put. -->
<section
	class="[content-visibility:auto]"
	style="contain-intrinsic-size: auto {height};"
>
	{#if title || onMore}
		<SectionHeading title={title ?? ''} icon={icon ?? ICONS[mode]} {onMore}>
			{#if mostlySongs}
				<button
					onclick={playAll}
					class="flex shrink-0 cursor-pointer items-center gap-1 rounded-full bg-primary/10 px-2.5 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/20"
				>
					<HugeiconsIcon icon={PlayIcon} class="h-3.5 w-3.5" />
					{t('common.play_all')}
				</button>
			{/if}
		</SectionHeading>
	{/if}
	<!-- Measure on pointer enter, because a shelf skipped by content-visibility has no layout at
	     mount: scrollWidth reads 0 and the arrows never appear. They only show on hover, so measuring
	     as the pointer arrives is both correct and later than the mount-time forced layout.
	     An attachment rather than onpointerenter: the handler doesn't make this div interactive. -->
	<div class="group/shelf relative" {@attach measureOnEnter}>
		<div
			class="rail flex snap-x overflow-x-auto pb-2 {community ? 'gap-3' : 'gap-2'}"
			bind:this={row}
			onscroll={update}
		>
			{#each items as item, i (item.id + ':' + i)}
				<!-- A shelf keeps its form even where one item doesn't fit it: a stray song in an
				     artist shelf gets the plain card and its width, not a poster it isn't. -->
				{@const own = community ? item.kind === 'playlist' : item.kind === mode}
				<!-- min-w-0: a flex item's automatic minimum size is its min-content, which overrides
				     the basis, so without this a card with a long title grows past its slot. -->
				<div
					class="min-w-0 shrink-0 snap-start {own && community
						? 'basis-full sm:basis-[calc((100%-0.75rem)/2)] lg:basis-[calc((100%-2.25rem)/4)]'
						: ''}"
					style={own && community ? undefined : cardWidth(own && mode === 'playlist')}
				>
					{#if !own}
						<MediaCard {item} />
					{:else if community}
						<CommunityCard {item} />
					{:else if mode === 'artist'}
						<PortraitCard {item} />
					{:else if mode === 'playlist'}
						<StackCard {item} />
					{:else}
						<MediaCard {item} />
					{/if}
				</div>
			{/each}
		</div>
		<!-- Fades, not just arrows: a card sliced by the edge should read as "the row continues", which
		     is also what makes the arrow legible sitting on top of artwork. Both are pointer-transparent
		     so they never eat a click meant for the card underneath. -->
		{#if canLeft}
			<div
				class="pointer-events-none absolute inset-y-0 left-0 w-16 bg-gradient-to-r from-background to-transparent"
			></div>
			<button
				aria-label={t('a11y.scroll_left')}
				onclick={() => page(-1)}
				class="absolute left-1 top-1/2 flex h-9 w-9 -translate-y-1/2 cursor-pointer items-center justify-center rounded-full border bg-background text-foreground opacity-0 shadow-lg transition hover:scale-105 focus-visible:opacity-100 group-hover/shelf:opacity-100"
			>
				<HugeiconsIcon icon={ArrowLeft01Icon} class="h-4 w-4" />
			</button>
		{/if}
		{#if canRight}
			<div
				class="pointer-events-none absolute inset-y-0 right-0 w-16 bg-gradient-to-l from-background to-transparent"
			></div>
			<button
				aria-label={t('a11y.scroll_right')}
				onclick={() => page(1)}
				class="absolute right-1 top-1/2 flex h-9 w-9 -translate-y-1/2 cursor-pointer items-center justify-center rounded-full border bg-background text-foreground opacity-0 shadow-lg transition hover:scale-105 focus-visible:opacity-100 group-hover/shelf:opacity-100"
			>
				<HugeiconsIcon icon={ArrowRight01Icon} class="h-4 w-4" />
			</button>
		{/if}
	</div>
</section>
