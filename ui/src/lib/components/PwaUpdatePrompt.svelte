<script lang="ts">
	// Registers the service worker and shows an update-available banner — but only in a plain
	// browser tab. Inside the Tauri webview a service worker is unwanted (untested against that
	// engine, and there's nothing for it to do there), so this component is simply never mounted
	// there — see +layout.svelte.
	import { useRegisterSW } from 'virtual:pwa-register/svelte';
	import { Button } from '$lib/components/ui/button';

	const { needRefresh, updateServiceWorker } = useRegisterSW({
		onRegisterError: (error) => console.error('service worker registration failed', error)
	});

	let updating = $state(false);
	async function applyUpdate() {
		updating = true;
		await updateServiceWorker(true);
	}
</script>

{#if $needRefresh}
	<div
		class="fixed bottom-24 left-1/2 z-[100] flex -translate-x-1/2 items-center gap-3 rounded-lg border bg-card px-4 py-2 text-sm shadow-lg"
	>
		<span>A new version is available.</span>
		<Button size="sm" onclick={applyUpdate} disabled={updating}>
			{updating ? 'Updating…' : 'Reload to update'}
		</Button>
	</div>
{/if}
