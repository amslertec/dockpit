<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import { formatDateTime } from '$lib/utils/format';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Button from '$lib/components/ui/Button.svelte';

	const containerId = $derived($page.params.id);

	let data = $state<any>(null);
	let loading = $state(true);
	let error = $state('');

	onMount(() => load());

	async function load() {
		if (!$selectedEnv || !containerId) return;
		loading = true;
		const r = await api.get<any>(`/env/${$selectedEnv}/containers/${containerId}/inspect`);
		if (r.success && r.data) {
			data = r.data;
		} else {
			error = r.error || 'Container not found';
		}
		loading = false;
	}

	// Derived values from inspect data
	const name = $derived(data?.Name?.replace(/^\//, '') || '');
	const status = $derived(data?.State?.Status || '');
	const running = $derived(data?.State?.Running || false);
	const startedAt = $derived(data?.State?.StartedAt || '');
	const createdAt = $derived(data?.Created || '');
	const image = $derived(data?.Config?.Image || '');
	const imageId = $derived(data?.Image || '');
	const cmd = $derived(data?.Config?.Cmd || []);
	const entrypoint = $derived(data?.Config?.Entrypoint || []);
	const workingDir = $derived(data?.Config?.WorkingDir || '');
	const envVars = $derived(data?.Config?.Env || []);
	const labels = $derived(data?.Config?.Labels || {});
	const restartPolicy = $derived(data?.HostConfig?.RestartPolicy || {});
	const healthStatus = $derived(data?.State?.Health?.Status || '');
	const healthFailCount = $derived(data?.State?.Health?.FailingStreak || 0);
	const healthLog = $derived(data?.State?.Health?.Log || []);
	const mounts = $derived(data?.Mounts || []);
	const networks = $derived(data?.NetworkSettings?.Networks || {});
	const ports = $derived(data?.NetworkSettings?.Ports || {});

	function extractHealth(s: string): string {
		if (s === 'healthy') return 'healthy';
		if (s === 'unhealthy') return 'unhealthy';
		if (s === 'starting') return 'starting';
		return '';
	}

	async function containerAction(action: string) {
		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${containerId}/action`, { action });
		if (r.success) { toasts.success(action); setTimeout(load, 500); }
		else toasts.error(r.error || $t('common.error'));
	}
</script>

<svelte:head><title>DockPit — {name || 'Container'}</title></svelte:head>

{#if loading}
	<div class="flex justify-center py-16"><div class="w-6 h-6 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
{:else if error}
	<div class="text-center py-16 text-sm text-[var(--red)]">{error}</div>
{:else if data}
	<!-- Header -->
	<div class="flex items-center justify-between mb-5 flex-wrap gap-3">
		<div class="flex items-center gap-3">
			<a href="/containers" class="w-8 h-8 flex items-center justify-center rounded-md border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition">
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>
			</a>
			<div>
				<h2 class="text-lg font-semibold text-primary">{name}</h2>
				<div class="flex items-center gap-2 mt-0.5">
					<Badge status={status} health={extractHealth(healthStatus)} />
					<span class="text-xs text-muted font-mono">{containerId.substring(0, 12)}</span>
				</div>
			</div>
		</div>
		<div class="flex items-center gap-2 flex-wrap">
			{#if !running}
				<button onclick={() => containerAction('start')} class="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--green)] text-[var(--green)] hover:bg-[var(--green)]/8 transition">
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>{$t('containers.start')}</button>
			{:else}
				<button onclick={() => containerAction('stop')} class="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--red)] text-[var(--red)] hover:bg-[var(--red)]/8 transition">
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>{$t('containers.stop')}</button>
			{/if}
			<button onclick={() => containerAction('restart')} class="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--yellow)] text-[var(--yellow)] hover:bg-[var(--yellow)]/8 transition">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/></svg>{$t('containers.restart')}</button>
			<a href="/containers/{containerId}/logs" class="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--accent)] text-[var(--accent)] hover:bg-[var(--accent)]/8 transition no-underline">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg>{$t('containers.logs')}</a>
			<a href="/containers/{containerId}/terminal" class="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--green)] text-[var(--green)] hover:bg-[var(--green)]/8 transition no-underline">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>{$t('containers.terminal')}</a>
		</div>
	</div>

	<div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
		<!-- Container Status -->
		<div class="bg-card border border-theme rounded-lg overflow-hidden">
			<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">{$t('containerDetail.status')}</h3></div>
			<div class="divide-y divide-[var(--border)]">
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">ID</span><span class="text-xs font-mono text-primary select-all">{data.Id?.substring(0, 24)}...</span></div>
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('common.name')}</span><span class="text-xs font-medium text-primary">{name}</span></div>
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('common.status')}</span><Badge status={status} health={extractHealth(healthStatus)} /></div>
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('containerDetail.created')}</span><span class="text-xs text-primary">{formatDateTime(createdAt)}</span></div>
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('containerDetail.started')}</span><span class="text-xs text-primary">{formatDateTime(startedAt)}</span></div>
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('containerDetail.restartPolicy')}</span><span class="text-xs text-primary">{restartPolicy.Name || '—'}{restartPolicy.MaximumRetryCount ? ` (max ${restartPolicy.MaximumRetryCount})` : ''}</span></div>
			</div>
		</div>

		<!-- Image -->
		<div class="bg-card border border-theme rounded-lg overflow-hidden">
			<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">Image</h3></div>
			<div class="divide-y divide-[var(--border)]">
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">Image</span><span class="text-xs font-mono text-primary break-all">{image}</span></div>
				<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">Image ID</span><span class="text-xs font-mono text-muted break-all">{imageId?.substring(0, 30)}...</span></div>
				{#if entrypoint?.length > 0}
					<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">Entrypoint</span><span class="text-xs font-mono text-primary">{entrypoint.join(' ')}</span></div>
				{/if}
				{#if cmd?.length > 0}
					<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">CMD</span><span class="text-xs font-mono text-primary">{cmd.join(' ')}</span></div>
				{/if}
				{#if workingDir}
					<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">Working Dir</span><span class="text-xs font-mono text-primary">{workingDir}</span></div>
				{/if}
			</div>
		</div>

		<!-- Health Check -->
		{#if healthStatus}
			<div class="bg-card border border-theme rounded-lg overflow-hidden">
				<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">{$t('containerDetail.health')}</h3></div>
				<div class="divide-y divide-[var(--border)]">
					<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('common.status')}</span>
						<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium {healthStatus === 'healthy' ? 'bg-[var(--green-bg)] text-[var(--green)]' : healthStatus === 'unhealthy' ? 'bg-[var(--red-bg)] text-[var(--red)]' : 'bg-[var(--yellow-bg)] text-[var(--yellow)]'}">{healthStatus}</span>
					</div>
					<div class="flex justify-between px-4 py-2.5"><span class="text-xs text-muted">{$t('containerDetail.failCount')}</span><span class="text-xs text-primary">{healthFailCount}</span></div>
					{#if healthLog.length > 0}
						{@const lastLog = healthLog[healthLog.length - 1]}
						<div class="px-4 py-2.5">
							<span class="text-xs text-muted block mb-1">{$t('containerDetail.lastOutput')}</span>
							<div class="bg-[var(--bg-0)] rounded p-2 text-[10px] font-mono text-secondary max-h-[80px] overflow-y-auto break-all">{lastLog.Output || '—'}</div>
						</div>
					{/if}
				</div>
			</div>
		{/if}

		<!-- Ports -->
		{#if Object.keys(ports).length > 0}
			<div class="bg-card border border-theme rounded-lg overflow-hidden">
				<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">Ports</h3></div>
				<div class="divide-y divide-[var(--border)]">
					{#each Object.entries(ports) as [containerPort, bindings]}
						<div class="flex justify-between px-4 py-2.5">
							<span class="text-xs font-mono text-muted">{containerPort}</span>
							<span class="text-xs font-mono text-primary">
								{#if bindings && bindings.length > 0}
									{bindings.map((b: any) => `${b.HostIp || '0.0.0.0'}:${b.HostPort}`).join(', ')}
								{:else}
									—
								{/if}
							</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Environment Variables -->
		{#if envVars.length > 0}
			<div class="bg-card border border-theme rounded-lg overflow-hidden lg:col-span-2">
				<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">{$t('containerDetail.env')} ({envVars.length})</h3></div>
				<div class="divide-y divide-[var(--border)] max-h-[300px] overflow-y-auto">
					{#each envVars as env}
						{@const [key, ...valParts] = env.split('=')}
						{@const val = valParts.join('=')}
						<div class="flex gap-4 px-4 py-2">
							<span class="text-xs font-mono text-[var(--accent)] shrink-0 min-w-[140px]">{key}</span>
							<span class="text-xs font-mono text-secondary break-all">{val}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Volumes / Mounts -->
		{#if mounts.length > 0}
			<div class="bg-card border border-theme rounded-lg overflow-hidden lg:col-span-2">
				<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">{$t('containerDetail.volumes')} ({mounts.length})</h3></div>
				<div class="overflow-x-auto">
					<table class="w-full">
						<thead><tr class="border-b border-theme">
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containerDetail.type')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containerDetail.source')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containerDetail.destination')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containerDetail.mode')}</th>
						</tr></thead>
						<tbody>
							{#each mounts as m}
								<tr class="border-b border-theme last:border-0">
									<td class="px-4 py-2 text-xs"><span class="px-1.5 py-0.5 rounded text-[9px] font-medium uppercase {m.Type === 'bind' ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'bg-[var(--purple-bg)] text-[var(--purple)]'}">{m.Type}</span></td>
									<td class="px-4 py-2 text-xs font-mono text-secondary break-all">{m.Source || m.Name || '—'}</td>
									<td class="px-4 py-2 text-xs font-mono text-primary">{m.Destination}</td>
									<td class="px-4 py-2 text-xs text-muted">{m.RW ? 'rw' : 'ro'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}

		<!-- Labels -->
		{#if Object.keys(labels).length > 0}
			<div class="bg-card border border-theme rounded-lg overflow-hidden lg:col-span-2">
				<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">Labels ({Object.keys(labels).length})</h3></div>
				<div class="divide-y divide-[var(--border)] max-h-[250px] overflow-y-auto">
					{#each Object.entries(labels) as [key, val]}
						<div class="flex gap-4 px-4 py-2">
							<span class="text-xs font-mono text-[var(--purple)] shrink-0 min-w-[200px] break-all">{key}</span>
							<span class="text-xs font-mono text-secondary break-all">{val}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Connected Networks -->
		{#if Object.keys(networks).length > 0}
			<div class="bg-card border border-theme rounded-lg overflow-hidden lg:col-span-2">
				<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">{$t('containerDetail.networks')}</h3></div>
				<div class="overflow-x-auto">
					<table class="w-full">
						<thead><tr class="border-b border-theme">
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containerDetail.network')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">IP</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">Gateway</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">MAC</th>
						</tr></thead>
						<tbody>
							{#each Object.entries(networks) as [netName, net]}
								<tr class="border-b border-theme last:border-0">
									<td class="px-4 py-2 text-xs font-medium text-primary">{netName}</td>
									<td class="px-4 py-2 text-xs font-mono text-secondary">{net.IPAddress || '—'}</td>
									<td class="px-4 py-2 text-xs font-mono text-muted">{net.Gateway || '—'}</td>
									<td class="px-4 py-2 text-xs font-mono text-muted">{net.MacAddress || '—'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}
	</div>
{/if}
