<script module lang="ts">
	// Shared with every filter-chip row (Home's mood chips, Search's category chips): a faint tint
	// at rest, the accent when on, so the one active filter is the only saturated thing above the
	// feed. Tinted rather than outlined: a row of outlines was another dozen lines on a page that
	// had too many (#319), and a tint of the foreground reads on the artwork and on a plain page.
	export const chipClass = (active: boolean) =>
		`shrink-0 cursor-pointer rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors ${
			active
				? 'bg-primary text-primary-foreground'
				: 'bg-foreground/5 text-foreground/75 hover:bg-foreground/10 hover:text-foreground'
		}`;
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';

	// Previously duplicated inline in Home's and Search's filter rows (#mobile-friendly audit):
	// same `.rail` scroll container, same `chipClass`, but neither side had any indication that the
	// row scrolls — on a phone-width screen with more chips than fit, the row just stopped at the
	// edge with nothing suggesting there was more. The two edge fades below read scroll position and
	// disappear once there's nothing left in that direction to scroll to, so the affordance matches
	// what's actually possible rather than always showing both edges.
	let {
		children,
		class: className = '',
		...rest
	}: { children: Snippet; class?: string; [key: string]: unknown } = $props();

	let el = $state<HTMLDivElement | null>(null);
	let atStart = $state(true);
	let atEnd = $state(true);

	function updateEdges() {
		if (!el) return;
		atStart = el.scrollLeft <= 1;
		atEnd = el.scrollLeft + el.clientWidth >= el.scrollWidth - 1;
	}

	// Recompute on mount, when the container itself resizes (orientation change, sidebar toggle),
	// and when the chip list's contents change (e.g. Home's chips arriving after the feed's async
	// load) — any of these can turn a non-overflowing row into an overflowing one or back, and the
	// fade needs to reflect that without waiting for the user to scroll first.
	$effect(() => {
		if (!el) return;
		updateEdges();
		const ro = new ResizeObserver(updateEdges);
		ro.observe(el);
		const mo = new MutationObserver(updateEdges);
		mo.observe(el, { childList: true, subtree: true });
		return () => {
			ro.disconnect();
			mo.disconnect();
		};
	});
</script>

<div class="relative min-w-0">
	<div
		bind:this={el}
		onscroll={updateEdges}
		class="rail flex gap-2 overflow-x-auto {className}"
		{...rest}
	>
		{@render children()}
	</div>
	{#if !atStart}
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-y-0 left-0 w-8 bg-gradient-to-r from-background to-transparent"
		></div>
	{/if}
	{#if !atEnd}
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-y-0 right-0 w-8 bg-gradient-to-l from-background to-transparent"
		></div>
	{/if}
</div>
