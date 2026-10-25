<script lang="ts">
	// Mobile-reachable Profile page (mint-ui-redesign, specs/account-profile) — behavior that
	// previously only existed inside the desktop sidebar's AccountMenu.svelte dropdown. Opened via
	// the avatar button in HomeHero.svelte's header (web build; desktop keeps using AccountMenu).
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { auth, toast } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';
	import { t } from '$lib/i18n.svelte';
	import { Button } from '$lib/components/ui/button';
	import SignInGate from '$lib/components/SignInGate.svelte';

	let accounts = $state<api.SavedAccount[]>([]);
	let busy = $state(false);
	let addingAccount = $state(false);
	const others = $derived(accounts.filter((a) => !a.active));

	onMount(load);

	async function load() {
		try {
			accounts = await api.getGoogleAccounts();
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function chooseAccount(account: api.SavedAccount) {
		if (busy) return;
		busy = true;
		try {
			await api.switchGoogleAccount(account.id);
			await load();
		} catch (e) {
			toast.error(String(e));
		} finally {
			busy = false;
		}
	}

	async function doSignOut() {
		if (busy) return;
		busy = true;
		try {
			await api.signOut();
			// No redirect here: `auth-changed` flips `auth.account.signedIn` to false, and the root
			// layout's own gate check (specs/web-signin-gate) takes over the whole screen on its own.
		} catch (e) {
			toast.error(String(e));
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>Profile — {t('nav.account')}</title></svelte:head>

{#if addingAccount}
	<SignInGate onBack={() => { addingAccount = false; load(); }} />
{:else}
	<div class="mx-auto flex max-w-md flex-col gap-6 p-4 pb-32">
		<div class="flex items-center gap-3">
			<button onclick={() => goto('/')} aria-label={t('common.back')} class="flex h-9 w-9 items-center justify-center rounded-full bg-muted">
				←
			</button>
			<h1 class="font-heading text-lg font-bold">{t('nav.account')}</h1>
		</div>

		<div class="flex items-center gap-3 rounded-2xl border bg-card p-4">
			{#if auth.account?.thumbnail}
				<img
					src={thumb(auth.account.thumbnail, 128)}
					alt=""
					style="width:3rem;height:3rem;max-width:none"
					class="shrink-0 rounded-full object-cover"
				/>
			{:else}
				<div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-primary text-lg font-bold text-primary-foreground">
					{(auth.account?.name ?? '?').slice(0, 1).toUpperCase()}
				</div>
			{/if}
			<div class="min-w-0 flex-1">
				<div class="truncate font-semibold">{auth.account?.name ?? t('nav.account')}</div>
				<div class="truncate text-sm text-muted-foreground">
					{auth.account?.email ?? auth.account?.handle ?? ''}
				</div>
			</div>
			<span class="rounded-full bg-primary/12 px-2.5 py-1 text-xs font-semibold text-primary">
				{t('nav.active_badge')}
			</span>
		</div>

		{#if others.length}
			<div>
				<h2 class="mb-2 text-sm font-semibold text-muted-foreground">{t('nav.saved_accounts_hint')}</h2>
				<div class="flex flex-col gap-1">
					{#each others as account (account.id)}
						<button
							class="flex items-center gap-3 rounded-xl px-2 py-2 text-left hover:bg-muted disabled:opacity-50"
							disabled={busy}
							onclick={() => chooseAccount(account)}
						>
							{#if account.thumbnail}
								<img src={thumb(account.thumbnail, 64)} alt="" class="h-9 w-9 shrink-0 rounded-full object-cover" />
							{:else}
								<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-muted text-sm font-bold">
									{account.name?.slice(0, 1).toUpperCase() ?? '?'}
								</div>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="truncate text-sm font-medium">{account.name}</div>
								{#if account.handle}<div class="truncate text-xs text-muted-foreground">{account.handle}</div>{/if}
							</div>
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<button
			class="flex items-center gap-2 rounded-xl px-2 py-2 text-left text-sm font-semibold text-primary hover:bg-muted"
			onclick={() => (addingAccount = true)}
		>
			<span class="text-base">+</span> {t('nav.add_account')}
		</button>

		<div class="border-t pt-4">
			<a href="/settings" class="flex items-center gap-2 rounded-xl px-2 py-2 text-sm font-medium hover:bg-muted">
				⚙️ {t('nav.settings')}
				<span class="ml-auto text-muted-foreground">›</span>
			</a>
			<Button variant="ghost" class="mt-1 w-full justify-start gap-2 text-destructive hover:text-destructive" onclick={doSignOut} disabled={busy}>
				🚪 {t('nav.sign_out')}
			</Button>
		</div>

		<div class="flex flex-col items-center gap-2 pt-2 text-center text-xs text-muted-foreground">
			<img src="/icons/icon.svg" alt="" width="36" height="36" class="rounded-[22%]" />
			{t('nav.multi_account_tagline')}
		</div>
	</div>
{/if}
