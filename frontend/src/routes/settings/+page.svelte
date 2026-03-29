<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import Tabs from '$lib/components/ui/Tabs.svelte';

	let activeTab = $state(0);
	let settings = $state<Record<string, string>>({});
	let loading = $state(true);
	let saving = $state(false);

	// Webhook
	let webhookUrl = $state('');
	let webhookEnabled = $state(false);
	let testingWebhook = $state(false);

	// Email
	let smtpHost = $state('');
	let smtpPort = $state('587');
	let smtpUser = $state('');
	let smtpPass = $state('');
	let smtpFrom = $state('');
	let smtpTo = $state('');
	let smtpTls = $state(true);

	// Update Monitor
	let updateInterval = $state('24');
	let updateEnabled = $state(false);

	// Timezone
	let timezone = $state('');

	const timezoneOptions = [
		{ value: 'UTC', label: 'UTC' },
		{ value: 'Europe/Zurich', label: 'Europe/Zurich (CET/CEST)' },
		{ value: 'Europe/Berlin', label: 'Europe/Berlin (CET/CEST)' },
		{ value: 'Europe/Vienna', label: 'Europe/Vienna (CET/CEST)' },
		{ value: 'Europe/London', label: 'Europe/London (GMT/BST)' },
		{ value: 'Europe/Paris', label: 'Europe/Paris (CET/CEST)' },
		{ value: 'Europe/Amsterdam', label: 'Europe/Amsterdam (CET/CEST)' },
		{ value: 'Europe/Rome', label: 'Europe/Rome (CET/CEST)' },
		{ value: 'Europe/Madrid', label: 'Europe/Madrid (CET/CEST)' },
		{ value: 'Europe/Stockholm', label: 'Europe/Stockholm (CET/CEST)' },
		{ value: 'US/Eastern', label: 'US/Eastern (EST/EDT)' },
		{ value: 'US/Central', label: 'US/Central (CST/CDT)' },
		{ value: 'US/Mountain', label: 'US/Mountain (MST/MDT)' },
		{ value: 'US/Pacific', label: 'US/Pacific (PST/PDT)' },
		{ value: 'Asia/Tokyo', label: 'Asia/Tokyo (JST)' },
		{ value: 'Asia/Shanghai', label: 'Asia/Shanghai (CST)' },
		{ value: 'Asia/Kolkata', label: 'Asia/Kolkata (IST)' },
		{ value: 'Australia/Sydney', label: 'Australia/Sydney (AEST/AEDT)' },
	];

	const tabs = [
		{ id: 0, label: $t('profile.general') },
		{ id: 1, label: $t('settings.updateMonitor') },
		{ id: 2, label: $t('settings.webhooks') },
		{ id: 3, label: $t('settings.email') },
	];

	onMount(async () => {
		const r = await api.get<{ settings: Record<string, string> }>('/settings');
		if (r.success && r.data) {
			settings = r.data.settings;
			webhookUrl = settings['webhook_url'] || '';
			webhookEnabled = settings['webhook_enabled'] === 'true';
			smtpHost = settings['smtp_host'] || '';
			smtpPort = settings['smtp_port'] || '587';
			smtpUser = settings['smtp_user'] || '';
			smtpPass = settings['smtp_pass'] || '';
			smtpFrom = settings['smtp_from'] || '';
			smtpTo = settings['smtp_to'] || '';
			smtpTls = settings['smtp_tls'] !== 'false';
			updateInterval = settings['update_interval'] || '24';
			updateEnabled = settings['update_enabled'] === 'true';
			timezone = settings['timezone'] || Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
		}
		loading = false;
	});

	async function save() {
		saving = true;
		const s: Record<string, string> = {
			webhook_url: webhookUrl, webhook_enabled: String(webhookEnabled),
			smtp_host: smtpHost, smtp_port: smtpPort, smtp_user: smtpUser, smtp_pass: smtpPass,
			smtp_from: smtpFrom, smtp_to: smtpTo, smtp_tls: String(smtpTls),
			update_interval: updateInterval, update_enabled: String(updateEnabled),
			timezone,
		};
		const r = await api.post<string>('/settings', { settings: s });
		saving = false;
		localStorage.setItem('dp_timezone', timezone);
		if (r.success) toasts.success($t('settings.saved'));
		else toasts.error(r.error || $t('common.error'));
	}

	async function testWebhook() {
		if (!webhookUrl) { toasts.error($t('settings.webhookRequired')); return; }
		testingWebhook = true;
		const r = await api.post<string>('/settings/webhook/test', { url: webhookUrl });
		testingWebhook = false;
		if (r.success) toasts.success($t('settings.testSent'));
		else toasts.error(r.error || $t('common.error'));
	}
</script>

<svelte:head><title>DockPit — {$t('settings.title')}</title></svelte:head>

<div>
	<div class="bg-card border border-theme rounded-lg overflow-hidden">
		<Tabs tabs={tabs} active={activeTab} onchange={(id) => activeTab = id} />

		<div class="p-5">
			<!-- General -->
			{#if activeTab === 0}
				<h3 class="text-sm font-semibold text-[var(--text)] mb-2">{$t('settings.timezone')}</h3>
				<p class="text-xs text-[var(--text-secondary)] mb-4">{$t('settings.timezoneDesc')}</p>
				<div class="max-w-md space-y-4">
					<CustomSelect
						options={timezoneOptions}
						value={timezone}
						onchange={(v) => { timezone = String(v); localStorage.setItem('dp_timezone', String(v)); }}
					/>
					<Button variant="primary" size="md" onclick={save} loading={saving}>{$t('common.save')}</Button>
				</div>

				<div class="border-t border-[var(--border)] pt-6 mt-6">
					<h3 class="text-sm font-semibold text-[var(--text)] mb-2">{$t('settings.prometheus')}</h3>
					<p class="text-xs text-[var(--text-secondary)] mb-3">{$t('settings.prometheusDesc')}</p>
					<div class="bg-[var(--bg-3)] border border-[var(--border)] rounded-[var(--radius-md)] p-3 font-mono text-[11px] text-[var(--text-secondary)] select-all break-all">
						http://&lt;your-server&gt;:5533/api/metrics
					</div>
					<p class="text-[10px] text-[var(--text-muted)] mt-2">{$t('settings.prometheusHint')}</p>
				</div>

			<!-- Update Monitor -->
			{:else if activeTab === 1}
				<h3 class="text-sm font-semibold text-primary mb-2">{$t('settings.autoCheck')}</h3>
				<p class="text-xs text-secondary mb-4">{$t('settings.autoCheckDesc')}</p>

				<div class="max-w-md space-y-4">
					<div class="flex items-center gap-3">
						<CustomCheckbox checked={updateEnabled} onchange={(v) => updateEnabled = v} label={$t('settings.enableMonitor')} />
						<p class="text-[11px] text-muted">{$t('settings.monitorDesc')}</p>
					</div>

					{#if updateEnabled}
						<div>
							<CustomSelect
								options={[
									{value: '6', label: $t('settings.every6h')},
									{value: '12', label: $t('settings.every12h')},
									{value: '24', label: $t('settings.every24h')},
									{value: '48', label: $t('settings.every48h')}
								]}
								value={updateInterval}
								onchange={(v) => updateInterval = String(v)}
							/>
						</div>
					{/if}

					<Button variant="primary" size="md" onclick={save} loading={saving}>{$t('common.save')}</Button>
				</div>

			<!-- Webhooks -->
			{:else if activeTab === 2}
				<h3 class="text-sm font-semibold text-primary mb-2">{$t('settings.webhookTitle')}</h3>
				<p class="text-xs text-secondary mb-4">{$t('settings.webhookDesc')}</p>

				<div class="max-w-md space-y-4">
					<CustomCheckbox checked={webhookEnabled} onchange={(v) => webhookEnabled = v} label={$t('settings.enableWebhook')} />

					{#if webhookEnabled}
						<div>
							<TextInput bind:value={webhookUrl} label={$t('settings.webhookUrl')} placeholder="https://hooks.slack.com/services/..." />
							<p class="text-[10px] text-muted mt-1">{$t('settings.webhookHint')}</p>
						</div>

						<div class="flex gap-2">
							<Button variant="primary" size="md" onclick={save} loading={saving}>{$t('common.save')}</Button>
							<Button variant="secondary" size="md" onclick={testWebhook} loading={testingWebhook}>{$t('settings.testWebhook')}</Button>
						</div>
					{/if}
				</div>

			<!-- Email -->
			{:else if activeTab === 3}
				<h3 class="text-sm font-semibold text-primary mb-2">{$t('settings.emailTitle')}</h3>
				<p class="text-xs text-secondary mb-4">{$t('settings.emailDesc')}</p>

				<div class="max-w-md space-y-3">
					<div class="grid grid-cols-2 gap-3">
						<TextInput bind:value={smtpHost} label={$t('settings.smtpServer')} placeholder="smtp.gmail.com" />
						<TextInput bind:value={smtpPort} label={$t('settings.port')} placeholder="587" />
					</div>
					<TextInput bind:value={smtpUser} label={$t('login.username')} placeholder="user@example.com" />
					<TextInput bind:value={smtpPass} label={$t('login.password')} placeholder="" type="password" />
					<TextInput bind:value={smtpFrom} label={$t('settings.sender')} placeholder="dockpit@example.com" />
					<div>
						<TextInput bind:value={smtpTo} label={$t('settings.recipient')} placeholder="admin@example.com" />
						<p class="text-[10px] text-muted mt-1">{$t('settings.recipientHint')}</p>
					</div>
					<CustomCheckbox checked={smtpTls} onchange={(v) => smtpTls = v} label={$t('settings.useTLS')} size="sm" />

					<Button variant="primary" size="md" onclick={save} loading={saving}>{$t('common.save')}</Button>
				</div>
			{/if}
		</div>
	</div>
</div>
