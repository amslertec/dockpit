<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import type { UserProfile, TotpSetupResponse } from '$lib/api/types';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import Tabs from '$lib/components/ui/Tabs.svelte';

	let profile = $state<UserProfile | null>(null);
	let activeTab = $state(1); // 0=General, 1=Password, 2=2FA

	// Password
	let curPw = $state('');
	let newPw = $state('');
	let newPw2 = $state('');
	let pwError = $state('');

	// TOTP
	let totpSetup = $state<TotpSetupResponse | null>(null);
	let totpCode = $state('');
	let totpError = $state('');
	let totpLoading = $state(false);
	let disableCode = $state('');
	let disableError = $state('');

	onMount(async () => {
		const r = await api.get<UserProfile>('/profile');
		if (r.success) profile = r.data!;
	});

	async function changePw(e: Event) {
		e.preventDefault();
		pwError = '';
		if (newPw !== newPw2) { pwError = $t('profile.passwordMismatch'); return; }
		const r = await api.post<string>('/profile/password', { current_password: curPw, new_password: newPw });
		if (r.success) { toasts.success($t('profile.passwordChanged')); curPw = ''; newPw = ''; newPw2 = ''; }
		else pwError = r.error || $t('common.error');
	}

	async function setupTotp() {
		totpLoading = true;
		totpError = '';
		const r = await api.post<TotpSetupResponse>('/profile/totp/setup', {});
		totpLoading = false;
		if (r.success && r.data) totpSetup = r.data;
		else totpError = r.error || $t('common.error');
	}

	async function verifyTotp(e: Event) {
		e.preventDefault();
		totpError = '';
		const r = await api.post<string>('/profile/totp/verify', { code: totpCode });
		if (r.success) {
			toasts.success($t('profile.2faActivated'));
			totpSetup = null;
			totpCode = '';
			if (profile) profile = { ...profile, totp_enabled: true };
		} else {
			totpError = r.error || $t('profile.invalidCode');
		}
	}

	async function disableTotp(e: Event) {
		e.preventDefault();
		disableError = '';
		const r = await api.post<string>('/profile/totp/disable', { code: disableCode });
		if (r.success) {
			toasts.success($t('profile.2faDeactivated'));
			disableCode = '';
			if (profile) profile = { ...profile, totp_enabled: false };
		} else {
			disableError = r.error || $t('profile.invalidCode');
		}
	}

	const tabs = [
		{ id: 0, label: $t('profile.general') },
		{ id: 1, label: $t('profile.password') },
		{ id: 2, label: $t('profile.2fa') },
	];

</script>

<svelte:head><title>DockPit — {$t('profile.title')}</title></svelte:head>

{#if profile}
	<div>
		<!-- Profile header -->
		<div class="bg-card border border-theme rounded-lg p-5 mb-4 flex items-center gap-4">
			<div class="w-14 h-14 rounded-2xl bg-accent-light flex items-center justify-center text-xl font-bold text-accent shrink-0">
				{profile.username[0].toUpperCase()}
			</div>
			<div>
				<div class="text-lg font-semibold text-primary">{profile.username}</div>
				<div class="text-xs text-secondary">{profile.role}</div>
				{#if profile.totp_enabled}
					<div class="flex items-center gap-1.5 mt-1">
						<span class="w-2 h-2 rounded-full bg-[var(--green)]"></span>
						<span class="text-[11px] text-green">{$t('profile.2faActive')}</span>
					</div>
				{/if}
			</div>
		</div>

		<!-- Tabs -->
		<div class="bg-card border border-theme rounded-lg overflow-hidden">
			<Tabs tabs={tabs} active={activeTab} onchange={(id) => activeTab = Number(id)} />

			<div class="p-5">
				<!-- Tab 0: General -->
				{#if activeTab === 0}
					<div class="text-center py-10">
						<svg class="w-12 h-12 mx-auto mb-3 opacity-20 text-secondary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>
						</svg>
						<p class="text-sm text-secondary font-medium">{$t('profile.comingSoon')}</p>
						<p class="text-xs text-muted mt-1">{$t('profile.comingSoonDesc')}</p>
					</div>

				<!-- Tab 1: Password -->
				{:else if activeTab === 1}
					<h3 class="text-sm font-semibold text-primary mb-4">{$t('profile.changePassword')}</h3>
					<form onsubmit={changePw} class="space-y-3 max-w-sm">
						<TextInput bind:value={curPw} type="password" label={$t('profile.currentPassword')} required id="pw0" />
						<TextInput bind:value={newPw} type="password" label={$t('profile.newPassword')} placeholder={$t('users.minChars', { count: '6' })} required id="pw1" />
						<TextInput bind:value={newPw2} type="password" label={$t('profile.confirmPw')} required id="pw2" />
						{#if pwError}<p class="text-[var(--red)] text-[11px] mt-1">{pwError}</p>{/if}
						<Button variant="primary" size="md" type="submit">{$t('profile.changePassword')}</Button>
					</form>

				<!-- Tab 2: 2FA -->
				{:else if activeTab === 2}
					{#if profile.totp_enabled}
						<!-- 2FA is active -->
						<div class="flex items-start gap-3 mb-5 p-3 bg-green-light rounded-lg">
							<svg class="w-5 h-5 text-green shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
							<div>
								<p class="text-sm font-medium text-green">{$t('profile.2faEnabled')}</p>
								<p class="text-xs text-secondary mt-0.5">{$t('profile.2faProtected')}</p>
							</div>
						</div>

						<h4 class="text-sm font-semibold text-primary mb-3">{$t('profile.disable2FA')}</h4>
						<form onsubmit={disableTotp} class="max-w-sm space-y-3">
							<TextInput bind:value={disableCode} label={$t('profile.current2FACode')} placeholder={$t('login.2faPlaceholder')} required maxlength={6} id="dc" class="max-w-[180px] font-mono tracking-widest text-center" />
							{#if disableError}<p class="text-[var(--red)] text-[11px] mt-1">{disableError}</p>{/if}
							<Button variant="danger" size="md" type="submit">{$t('profile.disable2FA')}</Button>
						</form>
					{:else}
						<!-- 2FA setup -->
						{#if !totpSetup}
							<div class="max-w-sm">
								<h3 class="text-sm font-semibold text-primary mb-2">{$t('profile.setup2FA')}</h3>
								<p class="text-xs text-secondary mb-4">{$t('profile.setup2FADesc')}</p>
								<Button variant="primary" size="md" onclick={setupTotp} loading={totpLoading}>{$t('profile.start2FA')}</Button>
								{#if totpError}<p class="text-[var(--red)] text-xs mt-2">{totpError}</p>{/if}
							</div>
						{:else}
							<!-- QR Code + Verify -->
							<div class="max-w-md">
								<h3 class="text-sm font-semibold text-primary mb-3">{$t('profile.scanQR')}</h3>
								<p class="text-xs text-secondary mb-4">{$t('profile.scanQRDesc')}</p>

								<div class="flex flex-col sm:flex-row gap-5 mb-5">
									<!-- QR Code -->
									<div class="bg-white p-3 rounded-lg self-start">
										<img src="data:image/png;base64,{totpSetup.qr_code}" alt="QR Code" class="w-40 h-40" />
									</div>

									<!-- Manual entry -->
									<div class="flex-1">
										<p class="text-[11px] text-muted mb-1">{$t('profile.manualEntry')}</p>
										<div class="bg-0 border border-theme rounded-md p-2 font-mono text-[11px] text-secondary break-all select-all mb-4">
											{totpSetup.secret}
										</div>

										<form onsubmit={verifyTotp} class="space-y-3">
											<TextInput bind:value={totpCode} label={$t('profile.verificationCode')} placeholder="000000" required maxlength={6} id="tc" class="max-w-[180px] font-mono tracking-widest text-center" />
											{#if totpError}<p class="text-[var(--red)] text-[11px] mt-1">{totpError}</p>{/if}
											<div class="flex gap-2">
												<Button variant="primary" size="md" type="submit">{$t('profile.confirmActivate')}</Button>
												<Button variant="secondary" size="md" onclick={() => totpSetup = null}>{$t('common.cancel')}</Button>
											</div>
										</form>
									</div>
								</div>
							</div>
						{/if}
					{/if}
				{/if}
			</div>
		</div>
	</div>
{/if}
