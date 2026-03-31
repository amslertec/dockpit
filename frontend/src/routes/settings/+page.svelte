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
	import { formatDateTime } from '$lib/utils/format';

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

	// Backup
	let backupDir = $state('/data/backups');
	let backupEnabled = $state(false);
	let backupDay = $state('daily');
	let backupTime = $state('02:00');
	let backupRetention = $state('7');
	let backups = $state<Array<{filename: string; size_bytes: number; created_at: string}>>([]);
	let creatingBackup = $state(false);
	let confirm = $state<{ message: string; action: () => void } | null>(null);
	let uploadFile = $state<File | null>(null);

	// Alert Rules
	let alertRules = $state<{id: number; name: string; enabled: boolean; event_match: string; action_type: string; last_triggered: string | null; trigger_count: number}[]>([]);
	let newRuleName = $state('');
	let newRuleEvent = $state('container_stop');
	let newRuleAction = $state('notify');
	let showAddRule = $state(false);

	const tabs = [
		{ id: 0, label: $t('profile.general') },
		{ id: 1, label: $t('settings.updateMonitor') },
		{ id: 2, label: $t('settings.webhooks') },
		{ id: 3, label: $t('settings.email') },
		{ id: 4, label: $t('settings.backup') },
		{ id: 5, label: $t('alerts.title') },
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
			backupDir = settings['backup_dir'] || '/data/backups';
			backupEnabled = settings['backup_enabled'] === 'true';
			backupDay = settings['backup_day'] || 'daily';
			backupTime = settings['backup_time'] || '02:00';
			backupRetention = settings['backup_retention'] || '7';
		}
		loading = false;
		loadBackups();
		loadAlertRules();
	});

	async function save() {
		saving = true;
		const s: Record<string, string> = {
			webhook_url: webhookUrl, webhook_enabled: String(webhookEnabled),
			smtp_host: smtpHost, smtp_port: smtpPort, smtp_user: smtpUser, smtp_pass: smtpPass,
			smtp_from: smtpFrom, smtp_to: smtpTo, smtp_tls: String(smtpTls),
			update_interval: updateInterval, update_enabled: String(updateEnabled),
			timezone,
			backup_dir: backupDir, backup_enabled: String(backupEnabled),
			backup_day: backupDay, backup_time: backupTime, backup_retention: backupRetention,
		};
		const r = await api.post<string>('/settings', { settings: s });
		saving = false;
		localStorage.setItem('dp_timezone', timezone);
		if (r.success) toasts.success($t('settings.saved'));
		else toasts.error(r.error || $t('common.error'));
	}

	async function loadBackups() {
		const r = await api.get<Array<{filename: string; size_bytes: number; created_at: string}>>('/backups');
		if (r.success && r.data) backups = r.data;
	}

	async function createBackup() {
		creatingBackup = true;
		const r = await api.post('/backups', {});
		creatingBackup = false;
		if (r.success) { toasts.success($t('settings.backupCreated')); loadBackups(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function deleteBackup(filename: string) {
		const r = await api.del(`/backups/${filename}`);
		if (r.success) { toasts.success('Deleted'); loadBackups(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function restoreBackup(filename: string) {
		const r = await api.post(`/backups/restore/${filename}`, {});
		if (r.success) toasts.success($t('settings.backupRestored'));
		else toasts.error(r.error || $t('common.error'));
	}

	async function downloadBackup(filename: string) {
		const token = localStorage.getItem('dp_token') || '';
		const res = await fetch(`/api/backups/${filename}`, { headers: { Authorization: `Bearer ${token}` } });
		if (!res.ok) { toasts.error('Download failed'); return; }
		const blob = await res.blob();
		const a = document.createElement('a');
		a.href = URL.createObjectURL(blob);
		a.download = filename;
		a.click();
		URL.revokeObjectURL(a.href);
	}

	async function handleUploadRestore() {
		if (!uploadFile) return;
		const formData = new FormData();
		formData.append('file', uploadFile);
		const token = localStorage.getItem('dp_token') || '';
		const res = await fetch('/api/backups/upload-restore', {
			method: 'POST',
			headers: { Authorization: `Bearer ${token}` },
			body: formData,
		});
		const data = await res.json();
		if (data.success) toasts.success($t('settings.backupRestored'));
		else toasts.error(data.error || $t('common.error'));
		uploadFile = null;
	}

	function formatBytes(bytes: number): string {
		if (bytes < 1024) return bytes + ' B';
		if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
		return (bytes / 1048576).toFixed(1) + ' MB';
	}

	async function testWebhook() {
		if (!webhookUrl) { toasts.error($t('settings.webhookRequired')); return; }
		testingWebhook = true;
		const r = await api.post<string>('/settings/webhook/test', { url: webhookUrl });
		testingWebhook = false;
		if (r.success) toasts.success($t('settings.testSent'));
		else toasts.error(r.error || $t('common.error'));
	}

	async function loadAlertRules() {
		const r = await api.get<{id: number; name: string; enabled: boolean; event_match: string; action_type: string; last_triggered: string | null; trigger_count: number}[]>('/alert-rules');
		if (r.success && r.data) alertRules = r.data;
	}

	async function addAlertRule() {
		if (!newRuleName.trim()) return;
		const r = await api.post<string>('/alert-rules', { name: newRuleName, event_match: newRuleEvent, action_type: newRuleAction });
		if (r.success) { newRuleName = ''; showAddRule = false; loadAlertRules(); toasts.success($t('settings.saved')); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function toggleAlertRule(id: number, enabled: boolean) {
		await api.put<string>(`/alert-rules/${id}`, { enabled });
		loadAlertRules();
	}

	async function deleteAlertRule(id: number) {
		const r = await api.del<string>(`/alert-rules/${id}`);
		if (r.success) alertRules = alertRules.filter(a => a.id !== id);
		else toasts.error(r.error || $t('common.error'));
	}

	function eventLabel(e: string): string {
		const map: Record<string, string> = { container_stop: $t('alerts.containerStop'), container_oom: $t('alerts.containerOom'), container_restart_loop: $t('alerts.containerRestart') };
		return map[e] || e;
	}

	function actionLabel(a: string): string {
		const map: Record<string, string> = { restart: $t('alerts.actionRestart'), notify: $t('alerts.actionNotify'), prune: $t('alerts.actionPrune') };
		return map[a] || a;
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
							<Button variant="warning" size="md" onclick={testWebhook} loading={testingWebhook}>{$t('settings.testWebhook')}</Button>
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

			<!-- Backup -->
			{:else if activeTab === 4}
				<h3 class="text-sm font-semibold text-primary mb-2">{$t('settings.backupTitle')}</h3>
				<p class="text-xs text-secondary mb-4">{$t('settings.backupDesc')}</p>

				<div class="max-w-lg space-y-4">
					<!-- Config -->
					<TextInput bind:value={backupDir} label={$t('settings.backupDir')} />
					<p class="text-[10px] text-muted -mt-2">{$t('settings.backupDirDesc')}</p>

					<div class="border-t border-theme pt-4">
						<h4 class="text-xs font-semibold text-primary mb-3">{$t('settings.backupSchedule')}</h4>
						<CustomCheckbox checked={backupEnabled} onchange={(v) => backupEnabled = v} label={$t('settings.backupEnable')} />

						{#if backupEnabled}
							<div class="grid grid-cols-3 gap-3 mt-3">
								<div>
									<label class="text-[11px] font-medium text-secondary block mb-1">{$t('settings.backupDay')}</label>
									<CustomSelect
										options={[
											{value: 'daily', label: $t('settings.backupDaily')},
											{value: '1', label: $t('settings.backupMonday')},
											{value: '2', label: $t('settings.backupTuesday')},
											{value: '3', label: $t('settings.backupWednesday')},
											{value: '4', label: $t('settings.backupThursday')},
											{value: '5', label: $t('settings.backupFriday')},
											{value: '6', label: $t('settings.backupSaturday')},
											{value: '7', label: $t('settings.backupSunday')},
										]}
										value={backupDay}
										onchange={(v) => backupDay = String(v)}
									/>
								</div>
								<div>
									<label class="text-[11px] font-medium text-secondary block mb-1">{$t('settings.backupTime')}</label>
									<input type="time" bind:value={backupTime} class="w-full px-3 py-2 rounded-[var(--radius-md)] border border-theme bg-[var(--bg-1)] text-[var(--text)] text-xs" />
								</div>
								<div>
									<label class="text-[11px] font-medium text-secondary block mb-1">{$t('settings.backupRetention')}</label>
									<div class="flex items-center gap-2">
										<input type="number" bind:value={backupRetention} min="1" max="100" class="w-20 px-3 py-2 rounded-[var(--radius-md)] border border-theme bg-[var(--bg-1)] text-[var(--text)] text-xs" />
										<span class="text-[11px] text-muted">{$t('settings.backupRetentionSuffix')}</span>
									</div>
								</div>
							</div>
						{/if}
					</div>

					<div class="flex gap-2">
						<Button variant="primary" size="md" onclick={save} loading={saving}>{$t('common.save')}</Button>
						<Button variant="success" size="md" onclick={createBackup} loading={creatingBackup}>{$t('settings.backupNow')}</Button>
					</div>
				</div>

				<!-- Backup List -->
				<div class="border-t border-theme pt-5 mt-6">
					<h4 class="text-xs font-semibold text-primary mb-3">{$t('settings.backupList')}</h4>
					{#if backups.length === 0}
						<p class="text-xs text-muted">{$t('settings.backupNoBackups')}</p>
					{:else}
						<div class="bg-card rounded-[var(--radius-lg)] border border-theme overflow-hidden">
							<table class="w-full text-xs">
								<thead>
									<tr class="border-b border-theme">
										<th class="text-left px-4 py-2.5 text-[10px] font-semibold uppercase tracking-wider text-muted">Filename</th>
										<th class="text-left px-3 py-2.5 text-[10px] font-semibold uppercase tracking-wider text-muted">{$t('settings.backupSize')}</th>
										<th class="text-left px-3 py-2.5 text-[10px] font-semibold uppercase tracking-wider text-muted">{$t('settings.backupDate')}</th>
										<th class="text-right px-4 py-2.5 text-[10px] font-semibold uppercase tracking-wider text-muted">{$t('common.actions')}</th>
									</tr>
								</thead>
								<tbody>
									{#each backups as b}
										<tr class="border-b border-theme last:border-0 hover:bg-[var(--bg-hover)] transition-colors">
											<td class="px-4 py-2.5 font-mono text-[11px] text-primary">{b.filename}</td>
											<td class="px-3 py-2.5 text-secondary">{formatBytes(b.size_bytes)}</td>
											<td class="px-3 py-2.5 text-secondary">{formatDateTime(b.created_at)}</td>
											<td class="px-4 py-2.5 text-right">
												<div class="flex items-center justify-end gap-1">
													<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition" title={$t('settings.backupDownload')} onclick={() => downloadBackup(b.filename)}>
															<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
														</button>
													<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition" title={$t('settings.backupRestore')} onclick={() => confirm = { message: $t('settings.backupRestoreConfirm'), action: () => { confirm = null; restoreBackup(b.filename); } }}>
															<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12a9 9 0 109-9"/><polyline points="3 3 3 9 9 9"/><path d="M3 9l3-3"/></svg>
														</button>
													<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('settings.backupDelete')} onclick={() => confirm = { message: $t('settings.backupDeleteConfirm'), action: () => { confirm = null; deleteBackup(b.filename); } }}>
															<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
														</button>
												</div>
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}
				</div>

				<!-- Upload Restore -->
				<div class="border-t border-theme pt-5 mt-6">
					<h4 class="text-xs font-semibold text-primary mb-2">{$t('settings.backupUpload')}</h4>
					<p class="text-[10px] text-muted mb-3">{$t('settings.backupUploadDesc')}</p>
					<div class="flex items-center gap-3">
						<input type="file" accept=".db" class="text-xs text-secondary file:mr-3 file:py-1.5 file:px-3 file:rounded-md file:border file:border-[var(--purple)]/40 file:text-xs file:font-medium file:bg-[var(--purple)]/8 file:text-[var(--purple)] hover:file:bg-[var(--purple)]/15 file:transition file:cursor-pointer" onchange={(e) => { const t = e.target as HTMLInputElement; uploadFile = t.files?.[0] || null; }} />
						{#if uploadFile}
							<Button variant="danger" size="sm" onclick={() => confirm = { message: $t('settings.backupRestoreConfirm'), action: () => { confirm = null; handleUploadRestore(); } }}>{$t('settings.backupRestore')}</Button>
						{/if}
					</div>
				</div>
			<!-- Alerts -->
			{:else if activeTab === 5}
				<h3 class="text-sm font-semibold text-primary mb-2">{$t('alerts.title')}</h3>
				<p class="text-xs text-secondary mb-4">{$t('alerts.noRules')}</p>

				{#if alertRules.length > 0}
					<div class="space-y-2 mb-4">
						{#each alertRules as rule}
							<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3">
								<div class="flex items-center justify-between gap-3">
									<div class="flex items-center gap-3 flex-1 min-w-0">
										<CustomCheckbox checked={rule.enabled} onchange={(v) => toggleAlertRule(rule.id, v)} size="sm" />
										<div class="min-w-0 flex-1">
											<div class="text-xs font-medium text-primary truncate">{rule.name}</div>
											<div class="text-[10px] text-muted">{eventLabel(rule.event_match)} &rarr; {actionLabel(rule.action_type)}</div>
											{#if rule.trigger_count > 0}
												<div class="text-[10px] text-muted mt-0.5">{$t('alerts.triggerCount')}: {rule.trigger_count} {$t('alerts.times')}{#if rule.last_triggered} &middot; {$t('alerts.lastTriggered')}: {formatDateTime(rule.last_triggered)}{/if}</div>
											{/if}
										</div>
									</div>
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => confirm = { message: $t('common.confirm') + '?', action: () => { confirm = null; deleteAlertRule(rule.id); } }}>
										<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
									</button>
								</div>
							</div>
						{/each}
					</div>
				{/if}

				{#if showAddRule}
					<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-4 space-y-3">
						<TextInput bind:value={newRuleName} label={$t('alerts.name')} placeholder="e.g. Auto-restart on crash" />
						<div class="grid grid-cols-2 gap-3">
							<div>
								<label class="block text-[11px] font-medium text-secondary mb-1">{$t('alerts.event')}</label>
								<CustomSelect
									options={[
										{ value: 'container_stop', label: $t('alerts.containerStop') },
										{ value: 'container_oom', label: $t('alerts.containerOom') },
										{ value: 'container_restart_loop', label: $t('alerts.containerRestart') },
									]}
									value={newRuleEvent}
									onchange={(v) => newRuleEvent = String(v)}
								/>
							</div>
							<div>
								<label class="block text-[11px] font-medium text-secondary mb-1">{$t('alerts.action')}</label>
								<CustomSelect
									options={[
										{ value: 'notify', label: $t('alerts.actionNotify') },
										{ value: 'restart', label: $t('alerts.actionRestart') },
										{ value: 'prune', label: $t('alerts.actionPrune') },
									]}
									value={newRuleAction}
									onchange={(v) => newRuleAction = String(v)}
								/>
							</div>
						</div>
						<div class="flex gap-2">
							<Button variant="primary" size="sm" onclick={addAlertRule}>{$t('common.save')}</Button>
							<Button variant="secondary" size="sm" onclick={() => { showAddRule = false; newRuleName = ''; }}>{$t('common.cancel')}</Button>
						</div>
					</div>
				{:else}
					<Button variant="primary" size="sm" onclick={() => showAddRule = true}>{$t('alerts.addRule')}</Button>
				{/if}
			{/if}
		</div>
	</div>
</div>

<!-- Confirm Dialog -->
{#if confirm}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 flex items-center justify-center p-4" onclick={(e) => { if (e.target === e.currentTarget) confirm = null; }}>
		<div class="bg-card border border-theme rounded-xl p-6 max-w-sm w-full shadow-xl">
			<p class="text-sm text-primary mb-4">{confirm.message}</p>
			<div class="flex justify-end gap-2">
				<Button variant="success" size="sm" onclick={() => confirm = null}>{$t('common.cancel')}</Button>
				<Button variant="danger" size="sm" onclick={confirm.action}>{$t('common.confirm')}</Button>
			</div>
		</div>
	</div>
{/if}
