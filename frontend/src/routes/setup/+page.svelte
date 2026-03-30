<script lang="ts">
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { auth } from '$lib/stores/auth';
	import type { LoginResponse, AppStatus } from '$lib/api/types';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import { t } from '$lib/i18n';

	let username = $state('');
	let password = $state('');
	let password2 = $state('');
	let error = $state('');
	let loading = $state(false);

	// If setup is already done, redirect to login
	onMount(async () => {
		const status = await api.get<AppStatus>('/status');
		if (status.success && status.data?.setup_complete) {
			goto('/login');
		}
	});

	async function submit(e: Event) {
		e.preventDefault();
		if (password !== password2) { error = $t('setup.passwordMismatch'); return; }
		loading = true;
		const r = await api.post<LoginResponse>('/setup', { username, password });
		loading = false;
		if (r.success && r.data) {
			auth.login(r.data.token, r.data.username);
			goto('/');
		} else {
			error = r.error || $t('common.error');
		}
	}
</script>

<div class="flex items-center justify-center min-h-screen bg-0 px-4">
	<div class="bg-card border border-theme rounded-2xl p-8 md:p-10 w-full max-w-sm shadow-2xl">
		<div class="text-center mb-8">
			<img src="/logo.svg" alt="DockPit" class="w-12 h-12 mx-auto mb-3" />
			<h1 class="text-2xl font-bold bg-gradient-to-r from-[var(--accent)] to-[var(--purple)] bg-clip-text text-transparent">DockPit</h1>
			<p class="text-secondary text-sm mt-1">{$t('setup.title')}</p>
		</div>
		<form onsubmit={submit} class="space-y-4">
			<TextInput id="su" bind:value={username} label={$t('setup.username')} placeholder={$t('setup.placeholderUser')} required />
			<TextInput id="sp1" type="password" bind:value={password} label={$t('setup.password')} placeholder={$t('setup.placeholderPw')} required />
			<TextInput id="sp2" type="password" bind:value={password2} label={$t('setup.confirmPassword')} placeholder={$t('setup.placeholderPw2')} required />
			{#if error}<p class="text-[var(--red)] text-xs">{error}</p>{/if}
			<Button variant="primary" size="lg" type="submit" loading={loading} class="w-full">{loading ? $t('setup.submitting') : $t('setup.submit')}</Button>
		</form>
	</div>
</div>
