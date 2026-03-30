<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import type { LoginResponse } from '$lib/api/types';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import { t, locale } from '$lib/i18n';

	let username = $state('');
	let password = $state('');
	let password2 = $state('');
	let error = $state('');
	let loading = $state(false);

	async function submit(e: Event) {
		e.preventDefault();
		if (password !== password2) { error = $t('setup.passwordMismatch'); return; }
		loading = true;
		const r = await api.post<LoginResponse>('/setup', { username, password });
		loading = false;
		if (r.success && r.data) {
			goto('/login');
		} else {
			error = r.error || $t('common.error');
		}
	}
</script>

<div class="setup-bg flex items-center justify-center min-h-screen px-4">
	<!-- Ambient glow -->
	<div class="fixed top-1/4 left-1/2 -translate-x-1/2 w-[500px] h-[500px] rounded-full bg-[var(--accent)] opacity-[0.04] blur-[120px] pointer-events-none"></div>

	<div class="setup-card w-full max-w-[400px] p-8 md:p-10 relative">
		<div class="absolute top-4 right-4">
			<button
				class="w-9 h-9 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] hover:border-[var(--border-light)] transition-all duration-200 text-base leading-none"
				onclick={() => locale.toggle()}
				title={$locale === 'en' ? 'Deutsch' : 'English'}
			>
				{#if $locale === 'en'}
					<svg class="w-5 h-5" viewBox="0 0 36 36"><path fill="#FFCD05" d="M0 27a4 4 0 004 4h28a4 4 0 004-4v-3H0v3z"/><path fill="#ED1F24" d="M0 12h36v12H0z"/><path fill="#141414" d="M32 5H4a4 4 0 00-4 4v3h36V9a4 4 0 00-4-4z"/></svg>
				{:else}
					<svg class="w-5 h-5" viewBox="0 0 36 36"><path fill="#00247D" d="M32 5H4a4 4 0 00-4 4v18a4 4 0 004 4h28a4 4 0 004-4V9a4 4 0 00-4-4z"/><path fill="#CF1B2B" d="M22.6 13.5L36 7.3V9l-9.7 4.5h-3.7zM36 27l-14.2-6.5h3.7L36 25v2zM0 9v-1.7l14.2 6.5H10.5L0 9.3V9zM0 27v-2l10.5-4.5h3.7L0 27z"/><path fill="#EEE" d="M36 14.5v7H20.5V31h-5V21.5H0v-7h15.5V5h5v9.5z"/><path fill="#CF1B2B" d="M36 16v4H19v11h-2V20H0v-4h17V5h2v11z"/></svg>
				{/if}
			</button>
		</div>
		<div class="text-center mb-8">
			<img src="/logo.png" alt="DockPit" class="w-[72px] h-[72px] mx-auto mb-4 rounded-[var(--radius-xl)]" />
			<h1 class="text-2xl font-bold bg-gradient-to-r from-[var(--accent)] via-[var(--purple)] to-[#00b4d8] bg-clip-text text-transparent">DockPit</h1>
			<p class="text-[var(--text-muted)] text-sm mt-1.5">{$t('setup.title')}</p>
		</div>
		<form onsubmit={submit} class="space-y-5">
			<TextInput id="su" bind:value={username} label={$t('setup.username')} placeholder={$t('setup.placeholderUser')} required />
			<TextInput id="sp1" type="password" bind:value={password} label={$t('setup.password')} placeholder={$t('setup.placeholderPw')} required />
			<TextInput id="sp2" type="password" bind:value={password2} label={$t('setup.confirmPassword')} placeholder={$t('setup.placeholderPw2')} required />
			{#if error}<p class="text-[var(--red)] text-xs">{error}</p>{/if}
			<Button variant="primary" size="lg" type="submit" loading={loading} class="w-full">{loading ? $t('setup.submitting') : $t('setup.submit')}</Button>
		</form>
	</div>
</div>

<style>
	.setup-bg {
		background: var(--bg-0);
		background-image:
			radial-gradient(at 20% 80%, rgba(108, 92, 231, 0.06) 0%, transparent 50%),
			radial-gradient(at 80% 20%, rgba(0, 180, 216, 0.04) 0%, transparent 50%);
	}
	.setup-card {
		background: var(--glass-bg);
		backdrop-filter: blur(24px) saturate(180%);
		-webkit-backdrop-filter: blur(24px) saturate(180%);
		border: 1px solid var(--glass-border);
		border-radius: var(--radius-xl);
		box-shadow: var(--shadow-lg), 0 0 80px rgba(108, 92, 231, 0.06);
	}
</style>
