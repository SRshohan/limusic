<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Add01Icon, Delete02Icon, DriveIcon, RefreshIcon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { supportsFsAccess } from '$lib/platform';
	import { addFiles, addFolder, local, rescan, removeSource } from '$lib/local/store.svelte';

	let fileInput = $state<HTMLInputElement | null>(null);
	let busy = $state(false);

	async function onPickFolder() {
		busy = true;
		await addFolder();
		busy = false;
	}

	async function onFilesChosen(e: Event) {
		const files = (e.target as HTMLInputElement).files;
		if (!files || !files.length) return;
		busy = true;
		await addFiles(files);
		busy = false;
		(e.target as HTMLInputElement).value = '';
	}

	async function onRescan(sourceId: string) {
		const source = local.sources.find((s) => s.id === sourceId);
		if (!source) return;
		busy = true;
		await rescan(source);
		busy = false;
	}

	async function onRemove(sourceId: string) {
		busy = true;
		await removeSource(sourceId);
		busy = false;
	}
</script>

<div class="rounded-xl border bg-card/40 p-4">
	<div class="mb-3 flex items-center justify-between gap-3">
		<div class="flex items-center gap-2 font-medium">
			<HugeiconsIcon icon={DriveIcon} class="h-4 w-4" /> Local sources
		</div>
		<div class="flex shrink-0 gap-2">
			{#if supportsFsAccess}
				<Button variant="outline" size="sm" class="gap-2" disabled={busy} onclick={onPickFolder}>
					<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /> Add folder
				</Button>
			{/if}
			<Button
				variant="outline"
				size="sm"
				class="gap-2"
				disabled={busy}
				onclick={() => fileInput?.click()}
			>
				<HugeiconsIcon icon={Add01Icon} class="h-4 w-4" /> Add files
			</Button>
			<input
				bind:this={fileInput}
				type="file"
				accept="audio/*"
				multiple
				class="hidden"
				onchange={onFilesChosen}
			/>
		</div>
	</div>

	{#if local.error}
		<p class="mb-2 text-sm text-destructive">{local.error}</p>
	{/if}

	{#if local.sources.length}
		<ul class="flex flex-col gap-1">
			{#each local.sources as source (source.id)}
				<li class="flex items-center justify-between gap-3 rounded-lg px-2 py-1.5 hover:bg-accent/10">
					<span class="truncate text-sm" title={source.name}>{source.name}</span>
					<div class="flex shrink-0 items-center gap-1">
						{#if source.kind === 'directory'}
							<button
								class="flex h-7 w-7 items-center justify-center rounded-full text-muted-foreground transition hover:bg-accent/20 hover:text-foreground"
								aria-label="Rescan"
								disabled={busy}
								onclick={() => onRescan(source.id)}
							>
								<HugeiconsIcon icon={RefreshIcon} class="h-4 w-4" />
							</button>
						{/if}
						<button
							class="flex h-7 w-7 items-center justify-center rounded-full text-muted-foreground transition hover:bg-destructive/10 hover:text-destructive"
							aria-label="Remove"
							disabled={busy}
							onclick={() => onRemove(source.id)}
						>
							<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" />
						</button>
					</div>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="text-sm text-muted-foreground">
			No local music yet — add a folder{supportsFsAccess ? '' : ' of'} files to get started.
		</p>
	{/if}
</div>
