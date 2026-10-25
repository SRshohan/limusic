<script lang="ts">
	// The web build's cookie-paste sign-in form, in two contexts (mint-ui-redesign,
	// specs/web-signin-gate + specs/account-profile):
	// - No `onBack` (the default): the mandatory gate, rendered by +layout.svelte in place of the
	//   whole app shell whenever `auth.account` is known and `!signedIn` — nowhere to go back to,
	//   nothing playable yet, so the big "Welcome" framing with no back button. Success arrives back
	//   here via the existing `auth-changed` event (`initWebApp`'s listener sets `auth.account`) —
	//   the root layout's own check then stops rendering this component on its own; nothing to do
	//   here beyond letting the request resolve.
	// - `onBack` passed: the "Add another account" flow from Profile (task 3.3) — assumes a session
	//   already exists, so a compact header with a back button instead of the welcome moment, and
	//   `onBack()` is called on success to return to Profile (the root gate is irrelevant here: the
	//   existing session is already signed in, so nothing there would react to a second account
	//   being added).
	// Both share the same form below: reuses the exact `signInWithCookie` call
	// `AccountMenu.svelte`'s desktop-sidebar version of this same flow already uses.
	import * as api from '$lib/api';
	import { toast } from '$lib/player.svelte';
	import { t } from '$lib/i18n.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Textarea } from '$lib/components/ui/textarea';

	let { onBack }: { onBack?: () => void } = $props();

	let cookieInput = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function submit() {
		const cookie = cookieInput.trim();
		if (!cookie || busy) return;
		busy = true;
		error = null;
		try {
			await api.signInWithCookie(cookie);
			// Gate mode: nothing further here — a successful sign-in fires `auth-changed`, which
			// `initWebApp`'s listener (player.svelte.ts) picks up and sets `auth.account`, and the
			// root layout's own check stops rendering this component. Add-account mode: there's
			// already a signed-in session, so that mechanism has nothing to react to — call back
			// explicitly instead.
			onBack?.();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			toast.error(error);
		} finally {
			busy = false;
		}
	}
</script>

<div class="flex h-screen flex-col items-center overflow-y-auto bg-background px-4 py-10 text-foreground">
	{#if onBack}
		<div class="mb-2 flex w-full max-w-sm items-center gap-3">
			<button
				class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-muted text-foreground"
				aria-label={t('common.back')}
				onclick={onBack}
			>
				←
			</button>
			<h1 class="font-heading text-lg font-bold">{t('nav.add_account')}</h1>
		</div>
	{:else}
		<div class="flex w-full max-w-sm flex-col items-center gap-3 text-center">
			<div
				class="flex h-16 w-16 items-center justify-center rounded-[22%] bg-gradient-to-br from-primary to-primary/70 text-primary-foreground"
			>
				<!-- Same equalizer-bar mark as the app icon, drawn inline rather than pulled in as an
				     <img> — this is the one place in the app that wants it standalone and large, with
				     no favicon/manifest indirection in between. -->
				<svg viewBox="0 0 512 512" class="h-9 w-9" fill="none" aria-hidden="true">
					<rect x="120" y="248" width="40" height="112" rx="20" fill="currentColor" />
					<rect x="186" y="200" width="40" height="160" rx="20" fill="currentColor" />
					<rect x="252" y="140" width="40" height="220" rx="20" fill="currentColor" />
					<rect x="318" y="180" width="40" height="180" rx="20" fill="currentColor" />
					<rect x="384" y="228" width="40" height="132" rx="20" fill="currentColor" />
				</svg>
			</div>
			<h1 class="font-heading text-xl font-bold">{t('signin.welcome')}</h1>
			<span class="rounded-full bg-primary/12 px-3 py-1 text-xs font-semibold text-primary">
				🔒 {t('signin.required_badge')}
			</span>
			<p class="text-sm text-muted-foreground">{t('signin.intro')}</p>
		</div>
	{/if}

	<div class="mt-6 w-full max-w-sm">
		<details class="mb-4 rounded-2xl bg-muted/60 p-4" open>
			<summary class="cursor-pointer text-sm font-semibold text-primary">
				{t('nav.sign_in_cookie_help_toggle')}
			</summary>
			<ol class="ml-4 mt-2 flex list-decimal flex-col gap-2 text-xs text-muted-foreground">
				<li>{t('nav.sign_in_cookie_step1')}</li>
				<li>{t('nav.sign_in_cookie_step2')}</li>
				<li>{t('nav.sign_in_cookie_step3')}</li>
				<li>{t('nav.sign_in_cookie_step4')}</li>
			</ol>
		</details>

		<label class="mb-2 block text-sm font-semibold" for="signin-cookie">
			{t('nav.sign_in_cookie_label')}
		</label>
		<Textarea
			id="signin-cookie"
			bind:value={cookieInput}
			placeholder={t('nav.sign_in_cookie_placeholder')}
			class="min-h-28 font-mono text-xs"
			disabled={busy}
		/>

		{#if error}
			<div class="mt-3 rounded-xl bg-destructive/10 px-3 py-2.5 text-sm text-destructive">
				⚠️ {error}
			</div>
		{/if}

		<Button class="mt-4 w-full rounded-full" disabled={!cookieInput.trim() || busy} onclick={submit}>
			{busy ? t('signin.checking') : t('signin.submit')}
		</Button>

		<p class="mt-4 text-xs text-muted-foreground">🔒 {t('signin.security_note')}</p>
	</div>
</div>
