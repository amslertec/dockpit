<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import Tabs from '$lib/components/ui/Tabs.svelte';
	import type { EnvironmentInfo, RegistryInfo } from '$lib/api/types';

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

	// Registry
	let registries = $state<RegistryInfo[]>([]);
	let regRegistry = $state('');
	let regUser = $state('');
	let regPass = $state('');
	let regLoading = $state(false);
	let regError = $state('');

	const tabs = [
		{ id: 0, label: $t('env.connectedServers') },
		{ id: 1, label: $t('env.connectRemote') },
		{ id: 2, label: $t('env.dockerLogin') },
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
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" title={$t('common.edit')} onclick={() => openEdit(env)}>
													<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
												</button>
												{#if !env.is_local}
													<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--red)] hover:border-[var(--red)] transition" title={$t('common.delete')} onclick={() => removeServer(env.id)}>
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
				<div class="mb-6">
					<div class="flex items-center gap-2 mb-2">
						<div class="w-5 h-5 rounded-full bg-accent-light text-accent flex items-center justify-center text-[10px] font-bold shrink-0">1</div>
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

				<div>
					<div class="flex items-center gap-2 mb-3">
						<div class="w-5 h-5 rounded-full bg-accent-light text-accent flex items-center justify-center text-[10px] font-bold shrink-0">2</div>
						<span class="text-xs font-medium text-primary">{$t('env.step2')}</span>
					</div>
					<form onsubmit={connect} class="ml-7 max-w-md space-y-3">
						<TextInput bind:value={addName} label={$t('env.nameOptional')} placeholder="e.g. Production Server" id="en" />
						<TextInput bind:value={addUrl} label={$t('env.agentAddress')} placeholder="http://192.168.1.100:5522" required id="eu" />
						{#if connectError}<p class="text-[var(--red)] text-xs">{connectError}</p>{/if}
						<Button variant="primary" size="md" type="submit" loading={connecting}>{connecting ? $t('env.connecting') : $t('env.connectServer')}</Button>
					</form>
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
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--red)] hover:border-[var(--red)] transition" title={$t('nav.logout')} onclick={() => removeRegistry(reg.registry)}>
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
				<Button variant="secondary" size="sm" onclick={() => editEnv = null}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" type="submit" loading={editSaving}>{editSaving ? $t('common.saving') : $t('common.save')}</Button>
			</div>
		</form>
	</Modal>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
