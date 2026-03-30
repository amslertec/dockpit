<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { auth } from '$lib/stores/auth';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import type { LoginResponse, EnvironmentInfo } from '$lib/api/types';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import { t, locale } from '$lib/i18n';

	let username = $state('');
	let password = $state('');
	let totpCode = $state('');
	let error = $state('');
	let loading = $state(false);
	let needs2FA = $state(false);

	async function submit(e: Event) {
		e.preventDefault();
		loading = true;
		error = '';
		const r = await api.post<LoginResponse>('/login', {
			username,
			password,
			totp_code: totpCode || null
		});
		if (r.success && r.data) {
			auth.login(r.data.token, r.data.username);
			const envR = await api.get<EnvironmentInfo[]>('/environments');
			if (envR.success && envR.data) {
				environments.set(envR.data);
				if (envR.data.length > 0) selectedEnv.select(envR.data[0].id);
			}
			goto('/');
		} else {
			const err = r.error || '';
			if (err.includes('2FA-Code erforderlich')) {
				needs2FA = true;
				error = '';
			} else {
				error = err || $t('login.invalidCredentials');
			}
		}
		loading = false;
	}
</script>

<div class="login-bg flex items-center justify-center min-h-screen px-4">
	<!-- Ambient glow -->
	<div class="fixed top-1/4 left-1/2 -translate-x-1/2 w-[500px] h-[500px] rounded-full bg-[var(--accent)] opacity-[0.04] blur-[120px] pointer-events-none"></div>

	<div class="login-card w-full max-w-[400px] p-8 md:p-10 relative">
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
			<div class="w-14 h-14 mx-auto mb-4 rounded-[var(--radius-xl)] bg-gradient-to-br from-[var(--accent)] to-[#00b4d8] flex items-center justify-center shadow-[var(--shadow-glow)]">
				<img src="/logo.png" alt="DockPit" class="w-8 h-8" />
			</div>
			<h1 class="text-2xl font-bold bg-gradient-to-r from-[var(--accent)] via-[var(--purple)] to-[#00b4d8] bg-clip-text text-transparent">DockPit</h1>
			<p class="text-[var(--text-muted)] text-sm mt-1.5">{$t('login.welcome')}</p>
		</div>
		<form onsubmit={submit} class="space-y-5">
			<TextInput id="lu" bind:value={username} label={$t('login.username')} required />
			<TextInput id="lp" type="password" bind:value={password} label={$t('login.password')} required />

			{#if needs2FA}
				<TextInput id="lt" bind:value={totpCode} label={$t('login.2faCode')} placeholder={$t('login.2faPlaceholder')} required maxlength={6} class="font-mono tracking-widest text-center" />
			{/if}

			{#if error}<p class="text-[var(--red)] text-xs">{error}</p>{/if}
			<Button variant="primary" size="lg" type="submit" loading={loading} class="w-full">{$t('login.submit')}</Button>
		</form>
	</div>
</div>

<style>
	.login-bg {
		background: var(--bg-0);
		background-image:
			radial-gradient(at 20% 80%, rgba(108, 92, 231, 0.06) 0%, transparent 50%),
			radial-gradient(at 80% 20%, rgba(0, 180, 216, 0.04) 0%, transparent 50%);
	}
	.login-card {
		background: var(--glass-bg);
		backdrop-filter: blur(24px) saturate(180%);
		-webkit-backdrop-filter: blur(24px) saturate(180%);
		border: 1px solid var(--glass-border);
		border-radius: var(--radius-xl);
		box-shadow: var(--shadow-lg), 0 0 80px rgba(108, 92, 231, 0.06);
	}
</style>
