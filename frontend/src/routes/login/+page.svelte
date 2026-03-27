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
				class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all duration-200 text-xs font-medium"
				onclick={() => locale.toggle()}
			>
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"/></svg>
			</button>
		</div>
		<div class="text-center mb-8">
			<div class="w-14 h-14 mx-auto mb-4 rounded-[var(--radius-xl)] bg-gradient-to-br from-[var(--accent)] to-[#00b4d8] flex items-center justify-center shadow-[var(--shadow-glow)]">
				<img src="/logo.svg" alt="DockPit" class="w-8 h-8 brightness-200" />
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
