<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import { formatDateTime } from '$lib/utils/format';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import Tabs from '$lib/components/ui/Tabs.svelte';
	import type { EnvironmentInfo, RegistryInfo, ScheduledJob } from '$lib/api/types';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';

	let activeTab = $state(0);
	let loading = $state(true);
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);

	// Edit modal
	let editEnv = $state<EnvironmentInfo | null>(null);
	let editName = $state('');
	let editUrl = $state('');
	let editSaving = $state(false);

	// Add server
	let addName = $state('');
	let addUrl = $state('');
	let connecting = $state(false);
	let connectError = $state('');

	// Scheduled Jobs
	let jobs = $state<ScheduledJob[]>([]);
	let jobsLoading = $state(true);
	let showAddJob = $state(false);
	let newJobEnv = $state('');
	let newJobType = $state('update_check');
	let newJobInterval = $state(24);
	let newJobStack = $state('');
	let creatingJob = $state(false);

	// Registry
	let registries = $state<RegistryInfo[]>([]);
	let regRegistry = $state('');
	let regUser = $state('');
	let regPass = $state('');
	let regLoading = $state(false);
	let regError = $state('');

	// Discovery
	let discovering = $state(false);
	let discoveredAgents = $state<{hostname: string; version: string; docker_version: string; paired: boolean; url: string}[]>([]);
	let scanSubnet = $state('');

	const tabs = [
		{ id: 0, label: $t('env.connectedServers') },
		{ id: 1, label: $t('env.connectRemote') },
		{ id: 2, label: $t('env.dockerLogin') },
		{ id: 3, label: $t('jobs.title') },
	];

	onMount(async () => {
		await loadAll();
	});

	async function loadAll() {
		loading = true;
		const [envR, regR] = await Promise.all([
			api.get<EnvironmentInfo[]>('/environments'),
			api.get<RegistryInfo[]>('/registries'),
		]);
		if (envR.success && envR.data) environments.set(envR.data);
		if (regR.success && regR.data) registries = regR.data;
		loading = false;
		loadJobs();

		// Check status of each remote server in background
		const envs = envR.success && envR.data ? envR.data : [];
		for (const env of envs) {
			if (!env.is_local) {
				api.get<string>(`/environments/${env.id}/status`).then(sr => {
					if (sr.success && sr.data) {
						environments.update(list => list.map(e => e.id === env.id ? { ...e, status: sr.data! } : e));
					}
				});
			}
		}
	}

	// Server edit
	function openEdit(env: EnvironmentInfo) { editEnv = env; editName = env.name; editUrl = env.url; }

	async function saveEdit(e: Event) {
		e.preventDefault();
		if (!editEnv) return;
		editSaving = true;
		const r = await api.put<string>(`/environments/${editEnv.id}`, { name: editName, url: editUrl });
		editSaving = false;
		if (r.success) { editEnv = null; toasts.success($t('env.updated')); loadAll(); }
		else toasts.error(r.error || $t('common.error'));
	}

	function removeServer(id: string) {
		confirmDlg = { message: $t('env.confirmRemove'), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/environments/${id}`);
			if (r.success) { toasts.success($t('env.removed')); loadAll(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	// Connect
	async function connect(e: Event) {
		e.preventDefault();
		connecting = true; connectError = '';
		const r = await api.post<EnvironmentInfo>('/environments', { name: addName, url: addUrl });
		connecting = false;
		if (r.success) { addName = ''; addUrl = ''; toasts.success($t('env.connected', { name: r.data!.name })); activeTab = 0; loadAll(); }
		else connectError = r.error || $t('common.error');
	}

	// Discovery
	async function discoverAgents() {
		discovering = true;
		discoveredAgents = [];
		const params = scanSubnet.trim() ? `?extra_subnet=${encodeURIComponent(scanSubnet.trim())}` : '';
		const r = await api.post<{hostname: string; version: string; docker_version: string; paired: boolean; url: string}[]>(`/agents/discover${params}`, {});
		discovering = false;
		if (r.success && r.data) {
			discoveredAgents = r.data.filter(a => !a.paired);
		}
		if (discoveredAgents.length === 0) {
			toasts.info($t('env.noAgentsFound'));
		}
	}

	// Connect discovered agent
	let connectAgent = $state<{hostname: string; url: string} | null>(null);
	let connectAgentName = $state('');

	async function doConnectAgent() {
		if (!connectAgent) return;
		connecting = true; connectError = '';
		const r = await api.post<EnvironmentInfo>('/environments', { name: connectAgentName || connectAgent.hostname, url: connectAgent.url });
		connecting = false;
		if (r.success) {
			toasts.success($t('env.connected', { name: r.data!.name }));
			discoveredAgents = discoveredAgents.filter(a => a.url !== connectAgent!.url);
			connectAgent = null; connectAgentName = '';
			activeTab = 0; loadAll();
		} else connectError = r.error || $t('common.error');
	}

	// Registry
	async function addRegistry(e: Event) {
		e.preventDefault();
		regLoading = true; regError = '';
		const r = await api.post<string>('/registries', { registry: regRegistry || 'docker.io', username: regUser, password: regPass });
		regLoading = false;
		if (r.success) { toasts.success(r.data || $t('env.loginSuccess')); regRegistry = ''; regUser = ''; regPass = ''; loadAll(); }
		else regError = r.error || $t('common.error');
	}

	function removeRegistry(registry: string) {
		confirmDlg = { message: $t('env.confirmLogout', { name: registry }), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/registries/${encodeURIComponent(registry)}`);
			if (r.success) { toasts.success($t('env.removed')); loadAll(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	// === Scheduled Jobs ===
	async function loadJobs() {
		jobsLoading = true;
		const r = await api.get<ScheduledJob[]>('/scheduled-jobs');
		if (r.success && r.data) jobs = r.data;
		jobsLoading = false;
	}

	async function createJob(e: Event) {
		e.preventDefault();
		creatingJob = true;
		const body: any = { env_id: newJobEnv, job_type: newJobType, interval_hours: newJobInterval };
		if (newJobType === 'stack_redeploy' && newJobStack) body.stack_name = newJobStack;
		const r = await api.post<ScheduledJob>('/scheduled-jobs', body);
		creatingJob = false;
		if (r.success) { showAddJob = false; toasts.success($t('jobs.created')); loadJobs(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function toggleJob(id: string, enabled: boolean) {
		await api.put<string>(`/scheduled-jobs/${id}`, { enabled });
		loadJobs();
	}

	async function deleteJob(id: string) {
		confirmDlg = { message: $t('jobs.confirmDelete'), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/scheduled-jobs/${id}`);
			if (r.success) { toasts.success($t('jobs.deleted')); loadJobs(); }
			else toasts.error(r.error || $t('common.error'));
		}};
	}

	async function runJob(id: string) {
		const r = await api.post<string>(`/scheduled-jobs/${id}/run`, {});
		if (r.success) toasts.success($t('jobs.started'));
		else toasts.error(r.error || $t('common.error'));
		setTimeout(loadJobs, 2000);
	}

	function envName(envId: string): string {
		return $environments.find(e => e.id === envId)?.name || envId;
	}

	function jobTypeLabel(type: string): string {
		const map: Record<string, string> = {
			'update_check': $t('jobs.updateCheck'),
			'system_prune': $t('jobs.systemPrune'),
			'stack_redeploy': $t('jobs.stackRedeploy'),
		};
		return map[type] || type;
	}

	function intervalLabel(hours: number): string {
		const map: Record<number, string> = {
			1: $t('jobs.every1h'), 6: $t('jobs.every6h'), 12: $t('jobs.every12h'),
			24: $t('jobs.every24h'), 48: $t('jobs.every48h'), 168: $t('jobs.weekly'),
		};
		return map[hours] || `${hours}h`;
	}
</script>

<svelte:head><title>DockPit — {$t('env.title')}</title></svelte:head>

<div>
	<!-- Tabs -->
	<div class="bg-card border border-theme rounded-lg overflow-hidden">
		<Tabs tabs={tabs} active={activeTab} onchange={(id) => activeTab = Number(id)} />

		<div class="p-5">
			<!-- Tab 0: Connected Servers -->
			{#if activeTab === 0}
				{#if loading}
					<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
				{:else}
					<div class="overflow-x-auto">
						<table class="w-full">
							<thead><tr class="border-b border-theme">
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.name')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('env.address')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('env.type')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold"></th>
							</tr></thead>
							<tbody>
								{#each $environments as env}
									<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
										<td class="px-4 py-2.5 text-sm font-medium text-primary">{env.name}</td>
										<td class="px-4 py-2.5 text-[11px] font-mono text-secondary">{env.is_local ? $t('env.local') : env.url}</td>
										<td class="px-4 py-2.5"><Badge status={env.status} /></td>
										<td class="px-4 py-2.5 text-xs text-secondary">{env.is_local ? $t('env.local') : $t('env.agent')}</td>
										<td class="px-4 py-2.5">
											<div class="flex gap-1">
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition" title={$t('common.edit')} onclick={() => openEdit(env)}>
													<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
												</button>
												{#if !env.is_local}
													<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('common.delete')} onclick={() => removeServer(env.id)}>
														<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
													</button>
												{/if}
											</div>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/if}

			<!-- Tab 1: Connect Remote Server -->
			{:else if activeTab === 1}
				<!-- Network Scan -->
				<div class="mb-6">
					<div class="flex items-center gap-2 mb-2">
						<div class="w-5 h-5 rounded-full bg-accent-light text-accent flex items-center justify-center text-[10px] font-bold shrink-0">1</div>
						<span class="text-xs font-medium text-primary">{$t('env.autoDiscovery')}</span>
					</div>
					<p class="text-xs text-secondary mb-3 ml-7">{$t('env.autoDiscoveryDesc')}</p>
					<div class="ml-7 flex items-end gap-3 flex-wrap">
						<Button variant="primary" size="md" onclick={discoverAgents} loading={discovering}>
							<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"/></svg>
							{discovering ? $t('env.scanning') : $t('env.scanNetwork')}
						</Button>
						<div class="flex items-end gap-2">
							<div>
								<label class="block text-[10px] text-muted mb-1">{$t('env.extraSubnet')}</label>
								<input
									type="text"
									bind:value={scanSubnet}
									placeholder="10.10.20"
									class="w-[130px] h-9 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-0)] text-[var(--text)] placeholder:text-[var(--text-muted)] focus:outline-none focus:border-[var(--accent)]"
								/>
							</div>
						</div>
					</div>

					{#if discoveredAgents.length > 0}
						<div class="mt-3 ml-7 space-y-2">
							{#each discoveredAgents as agent}
								<div class="flex items-center justify-between bg-[var(--bg-0)] border border-theme rounded-lg p-3">
									<div>
										<div class="text-sm font-medium text-primary">{agent.hostname}</div>
										<div class="text-[11px] text-muted">{agent.url} — Docker {agent.docker_version}</div>
									</div>
									<Button variant="primary" size="sm" onclick={() => { connectAgent = agent; connectAgentName = agent.hostname; }}>
										{$t('env.connectAgent')}
									</Button>
								</div>
							{/each}
						</div>
					{/if}
				</div>

				<!-- Manual Connect -->
				<div class="mb-6">
					<div class="flex items-center gap-2 mb-3">
						<div class="w-5 h-5 rounded-full bg-accent-light text-accent flex items-center justify-center text-[10px] font-bold shrink-0">2</div>
						<span class="text-xs font-medium text-primary">{$t('env.manualConnect')}</span>
					</div>
					<form onsubmit={connect} class="ml-7 max-w-md space-y-3">
						<TextInput bind:value={addName} label={$t('env.nameOptional')} placeholder="e.g. Production Server" id="en" />
						<TextInput bind:value={addUrl} label={$t('env.agentAddress')} placeholder="http://192.168.1.100:5522" required id="eu" />
						{#if connectError}<p class="text-[var(--red)] text-xs">{connectError}</p>{/if}
						<Button variant="primary" size="md" type="submit" loading={connecting}>{connecting ? $t('env.connecting') : $t('env.connectServer')}</Button>
					</form>
				</div>

				<!-- Agent Install Instructions -->
				<div class="pt-5 border-t border-theme">
					<div class="flex items-center gap-2 mb-2">
						<div class="w-5 h-5 rounded-full bg-[var(--bg-hover)] text-muted flex items-center justify-center text-[10px] font-bold shrink-0">?</div>
						<span class="text-xs font-medium text-primary">{$t('env.step1')}</span>
					</div>
					<div class="space-y-2 ml-7">
						<div>
							<p class="text-[11px] text-muted mb-1">Docker Run:</p>
							<div class="bg-0 border border-theme rounded-md p-2.5 font-mono text-[11px] text-secondary leading-relaxed overflow-x-auto whitespace-pre">docker run -d --name dockpit-agent \
  -p 5522:5522 \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -v /var/docker/container:/stacks \
  -e AGENT_STACKS_DIR=/stacks \
  --restart unless-stopped \
  amslertec/dockpit-agent:latest</div>
						</div>
						<div>
							<p class="text-[11px] text-muted mb-1">docker-compose.yml:</p>
							<div class="bg-0 border border-theme rounded-md p-2.5 font-mono text-[11px] text-secondary leading-relaxed overflow-x-auto whitespace-pre">services:
  dockpit-agent:
    image: amslertec/dockpit-agent:latest
    container_name: dockpit-agent
    restart: unless-stopped
    ports:
      - "5522:5522"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - /var/docker/container:/stacks
    environment:
      - AGENT_STACKS_DIR=/stacks</div>
						</div>
					</div>
				</div>
			<!-- Tab 2: Docker Login -->
			{:else if activeTab === 2}
				<div class="mb-6">
					<h3 class="text-sm font-semibold text-primary mb-2">{$t('env.savedLogins')}</h3>
					<p class="text-xs text-secondary mb-4">{$t('env.registryDesc')}</p>

					{#if registries.length > 0}
						<div class="overflow-x-auto mb-5">
							<table class="w-full">
								<thead><tr class="border-b border-theme">
									<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('env.registry')}</th>
									<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('login.username')}</th>
									<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold"></th>
								</tr></thead>
								<tbody>
									{#each registries as reg}
										<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
											<td class="px-4 py-2.5">
												<div class="flex items-center gap-2">
													<span class="w-2 h-2 rounded-full bg-[var(--green)]"></span>
													<span class="text-sm font-medium text-primary">{reg.registry}</span>
												</div>
											</td>
											<td class="px-4 py-2.5 text-xs text-secondary">{reg.username}</td>
											<td class="px-4 py-2.5">
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('nav.logout')} onclick={() => removeRegistry(reg.registry)}>
													<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
												</button>
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{:else}
						<div class="px-4 py-6 text-center text-sm text-muted bg-0 rounded-lg border border-theme mb-5">
							{$t('env.noLogins')}
						</div>
					{/if}
				</div>

				<div>
					<h3 class="text-sm font-semibold text-primary mb-3">{$t('env.newLogin')}</h3>
					<form onsubmit={addRegistry} class="max-w-md space-y-3">
						<div>
							<TextInput bind:value={regRegistry} label={$t('env.registry')} placeholder={$t('env.registryPlaceholder')} id="rr" />
							<p class="text-[10px] text-muted mt-1">{$t('env.registryHint')}</p>
						</div>
						<TextInput bind:value={regUser} label={$t('login.username')} required id="ru" />
						<div>
							<TextInput bind:value={regPass} label={$t('env.passwordToken')} type="password" required id="rp" />
							<p class="text-[10px] text-muted mt-1">{$t('env.tokenHint')}</p>
						</div>
						{#if regError}<p class="text-[var(--red)] text-xs">{regError}</p>{/if}
						<Button variant="primary" size="md" type="submit" loading={regLoading}>{regLoading ? $t('env.loggingIn') : $t('env.loginButton')}</Button>
					</form>
				</div>

			<!-- Tab 3: Scheduled Jobs -->
			{:else if activeTab === 3}
				<div class="flex items-center justify-between mb-4">
					<h3 class="text-sm font-semibold text-primary">{$t('jobs.title')}</h3>
					<Button variant="primary" size="sm" onclick={() => { newJobEnv = ''; newJobType = 'update_check'; newJobInterval = 24; newJobStack = ''; showAddJob = true; }}>{$t('jobs.addJob')}</Button>
				</div>

				{#if jobsLoading}
					<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
				{:else if jobs.length === 0}
					<div class="px-4 py-12 text-center bg-0 rounded-lg border border-theme">
						<svg class="w-10 h-10 mx-auto mb-3 text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
						<p class="text-sm font-medium text-primary mb-1">{$t('jobs.noJobs')}</p>
						<p class="text-xs text-muted">{$t('jobs.noJobsDesc')}</p>
					</div>
				{:else}
					<div class="overflow-x-auto">
						<table class="w-full">
							<thead><tr class="border-b border-theme">
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('updates.server')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('jobs.type')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('jobs.interval')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('jobs.lastRun')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('jobs.result')}</th>
								<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
							</tr></thead>
							<tbody>
								{#each jobs as job}
									<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
										<td class="px-4 py-2.5 text-sm font-medium text-primary">{envName(job.env_id)}</td>
										<td class="px-4 py-2.5 text-xs text-secondary">
											{jobTypeLabel(job.job_type)}
											{#if job.stack_name}<span class="text-muted ml-1">({job.stack_name})</span>{/if}
										</td>
										<td class="px-4 py-2.5 text-xs text-secondary">{intervalLabel(job.interval_hours)}</td>
										<td class="px-4 py-2.5 text-xs text-secondary">{job.last_run ? formatDateTime(job.last_run) : $t('jobs.never')}</td>
										<td class="px-4 py-2.5">
											{#if job.last_result === 'success'}
												<span class="inline-flex items-center gap-1 text-[11px] text-[var(--green)]">
													<span class="w-1.5 h-1.5 rounded-full bg-[var(--green)]"></span>
													{$t('jobs.success')}
												</span>
											{:else if job.last_result === 'error'}
												<span class="inline-flex items-center gap-1 text-[11px] text-[var(--red)]">
													<span class="w-1.5 h-1.5 rounded-full bg-[var(--red)]"></span>
													{$t('jobs.error')}
												</span>
											{:else}
												<span class="text-[11px] text-muted">—</span>
											{/if}
										</td>
										<td class="px-4 py-2.5">
											<div class="flex items-center gap-2">
												<CustomCheckbox checked={job.enabled} onchange={(val) => toggleJob(job.id, val)} size="sm" />
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition" title={$t('jobs.runNow')} onclick={() => runJob(job.id)}>
													<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
												</button>
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('common.delete')} onclick={() => deleteJob(job.id)}>
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
			{/if}
		</div>
	</div>
</div>

<!-- Edit Server Modal -->
{#if editEnv}
	<Modal title={$t('env.editServer')} onclose={() => editEnv = null}>
		<form onsubmit={saveEdit} class="space-y-3">
			<TextInput bind:value={editName} label={$t('common.name')} required id="edit-name" />
			<div>
				<TextInput bind:value={editUrl} label={$t('env.agentAddress')} required disabled={editEnv?.is_local} id="edit-url" />
				{#if editEnv?.is_local}<p class="text-[10px] text-muted mt-1">{$t('env.localSocket')}</p>{/if}
			</div>
			<div class="flex justify-end gap-2 pt-2">
				<Button variant="danger" size="sm" onclick={() => editEnv = null}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" type="submit" loading={editSaving}>{editSaving ? $t('common.saving') : $t('common.save')}</Button>
			</div>
		</form>
	</Modal>
{/if}

{#if showAddJob}
	<Modal title={$t('jobs.addJob')} onclose={() => showAddJob = false}>
		<form onsubmit={createJob} class="space-y-3">
			<div>
				<label class="block text-xs font-medium text-primary mb-1">{$t('updates.server')}</label>
				<CustomSelect
					options={$environments.map(e => ({ value: e.id, label: e.name }))}
					value={newJobEnv}
					onchange={(v) => newJobEnv = String(v)}
					placeholder={$t('jobs.selectServer')}
				/>
			</div>
			<div>
				<label class="block text-xs font-medium text-primary mb-1">{$t('jobs.type')}</label>
				<CustomSelect
					options={[
						{ value: 'update_check', label: $t('jobs.updateCheck') },
						{ value: 'system_prune', label: $t('jobs.systemPrune') },
						{ value: 'stack_redeploy', label: $t('jobs.stackRedeploy') },
					]}
					value={newJobType}
					onchange={(v) => newJobType = String(v)}
					placeholder={$t('jobs.selectType')}
				/>
			</div>
			<div>
				<label class="block text-xs font-medium text-primary mb-1">{$t('jobs.interval')}</label>
				<CustomSelect
					options={[
						{ value: 1, label: $t('jobs.every1h') },
						{ value: 6, label: $t('jobs.every6h') },
						{ value: 12, label: $t('jobs.every12h') },
						{ value: 24, label: $t('jobs.every24h') },
						{ value: 48, label: $t('jobs.every48h') },
						{ value: 168, label: $t('jobs.weekly') },
					]}
					value={newJobInterval}
					onchange={(v) => newJobInterval = Number(v)}
				/>
			</div>
			{#if newJobType === 'stack_redeploy'}
				<div>
					<TextInput bind:value={newJobStack} label={$t('jobs.selectStack')} placeholder="stack-name" id="job-stack" />
				</div>
			{/if}
			<div class="flex justify-end gap-2 pt-2">
				<Button variant="danger" size="sm" onclick={() => showAddJob = false}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" type="submit" loading={creatingJob}>{creatingJob ? $t('common.loading') : $t('common.create')}</Button>
			</div>
		</form>
	</Modal>
{/if}

{#if connectAgent}
	<Modal title={$t('env.connectAgent')} onclose={() => { connectAgent = null; connectError = ''; }}>
		<div class="space-y-4">
			<div>
				<p class="text-xs text-muted mb-1">{$t('env.agentAddress')}</p>
				<p class="text-sm font-medium text-primary">{connectAgent.url}</p>
				<p class="text-xs text-muted mt-1">Hostname: {connectAgent.hostname}</p>
			</div>
			<TextInput bind:value={connectAgentName} label={$t('common.name')} placeholder={connectAgent.hostname} id="can" />
			{#if connectError}<p class="text-[var(--red)] text-xs">{connectError}</p>{/if}
		</div>
		<div class="flex justify-end gap-2 mt-5">
			<Button variant="secondary" size="sm" onclick={() => { connectAgent = null; connectError = ''; }}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={doConnectAgent} loading={connecting}>{$t('env.connectServer')}</Button>
		</div>
	</Modal>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
