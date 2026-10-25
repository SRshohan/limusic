<script lang="ts">
	// The offline/local-music surface: fully self-contained, talks to IndexedDB/OPFS and the
	// browser playback engine only — never the Tauri backend, never a server. See
	// openspec/changes/offline-local-music-pwa.
	import OfflineImport from '$lib/components/offline/OfflineImport.svelte';
	import OfflineLibrary from '$lib/components/offline/OfflineLibrary.svelte';
	import OfflinePlaylists from '$lib/components/offline/OfflinePlaylists.svelte';
	import OfflineTransport from '$lib/components/offline/OfflineTransport.svelte';
</script>

<svelte:head><title>Offline music — SRMusic</title></svelte:head>

<div class="mx-auto flex max-w-3xl flex-col gap-5 p-4 pb-32">
	<div>
		<h1 class="font-heading text-xl font-bold">Offline music</h1>
		<p class="text-sm text-muted-foreground">
			Your own files, imported once and playable with no network at all.
		</p>
	</div>

	<OfflineImport />
	<OfflineLibrary />
	<OfflinePlaylists />
</div>

<!-- Fixed to the bottom like the main app's player bar, so transport controls stay reachable while
     scrolling a long library. -->
<div class="fixed inset-x-0 bottom-0 mx-auto max-w-3xl p-4">
	<OfflineTransport />
</div>
