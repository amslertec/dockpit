<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { extractHealth } from '$lib/utils/format';
	import { t } from '$lib/i18n';
	import type { EnvStats, SystemInfo, ContainerInfo } from '$lib/api/types';

	let stats = $state<EnvStats | null>(null);
	let sys = $state<SystemInfo | null>(null);
	let containers = $state<ContainerInfo[]>([]);
	let loading = $state(true);
	let serverName = $state('');
	const envStoreName = $derived($environments.find(e => e.id === $selectedEnv)?.name || '');

	onMount(() => load());
	$effect(() => { $selectedEnv; load(); });

	async function load() {
		if (!$selectedEnv) return;
		loading = true;
		const [sr, syr, cr, er] = await Promise.all([
			api.get<EnvStats>(`/env/${$selectedEnv}/stats`),
			api.get<SystemInfo>(`/env/${$selectedEnv}/system`),
			api.get<ContainerInfo[]>(`/env/${$selectedEnv}/containers`),
			api.get<import('$lib/api/types').EnvironmentInfo[]>('/environments'),
		]);
		if (sr.success) stats = sr.data!;
		if (syr.success) sys = syr.data!;
		if (cr.success) containers = (cr.data || []).slice(0, 10);
		if (er.success && er.data) {
			const env = er.data.find(e => e.id === $selectedEnv);
			if (env) serverName = env.name;
		}
		loading = false;
	}
</script>

<svelte:head><title>DockPit — Dashboard</title></svelte:head>

{#if loading}
	<div class="flex justify-center py-16"><div class="w-6 h-6 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
{:else if stats}
	{@const total = stats.containers_total || 1}
	{@const runPct = Math.round((stats.containers_running / total) * 100)}
	{@const healthy = containers.filter(c => c.status.includes('(healthy)')).length}
	{@const unhealthy = containers.filter(c => c.status.includes('(unhealthy)')).length}

	<!-- Server name header -->
	{#if sys}
		<div class="flex items-center gap-3 mb-4">
			<div class="w-8 h-8 rounded-lg bg-green-light flex items-center justify-center shrink-0">
				<img src="/logo.svg" alt="" class="w-5 h-5" />
			</div>
			<div>
				<h2 class="text-base font-semibold text-primary">{serverName || envStoreName || sys.hostname}</h2>
				<div class="flex items-center gap-2 text-[11px] text-secondary">
					<span>Docker {sys.docker_version}</span>
					<span>·</span>
					<span>{sys.cpus} CPU</span>
					<span>·</span>
					<span>{sys.memory_display}</span>
				</div>
			</div>
		</div>
	{/if}

	<!-- Stat cards -->
	<div class="grid grid-cols-2 md:grid-cols-4 xl:grid-cols-8 gap-3 mb-5">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{stats.containers_total}</div>
			<div class="text-[11px] text-secondary leading-tight">{$t('containers.title')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{stats.containers_running}</div>
			<div class="text-[11px] text-secondary leading-tight">{$t('dash.active')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-red">{stats.containers_stopped}</div>
			<div class="text-[11px] text-secondary leading-tight">{$t('dash.stopped')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{healthy}</div>
			<div class="text-[11px] text-secondary leading-tight">{$t('dash.healthy')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3 {unhealthy > 0 ? 'border-[var(--red)]' : ''}">
			<div class="text-xl font-bold {unhealthy > 0 ? 'text-red' : 'text-secondary'}">{unhealthy}</div>
			<div class="text-[11px] {unhealthy > 0 ? 'text-red' : 'text-secondary'} leading-tight">{$t('dash.unhealthy')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-purple">{stats.images_total}</div>
			<div class="text-[11px] text-secondary leading-tight">Images</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-yellow">{stats.volumes_total}</div>
			<div class="text-[11px] text-secondary leading-tight">Volumes</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{stats.networks_total}</div>
			<div class="text-[11px] text-secondary leading-tight">{$t('networks.title')}</div>
		</div>
	</div>

	<!-- Status + System -->
	<div class="grid grid-cols-1 md:grid-cols-3 gap-3 mb-5">
		<div class="md:col-span-2 bg-card border border-theme rounded-lg">
			<div class="px-4 py-3 border-b border-theme text-sm font-semibold text-primary">{$t('dash.containerStatus')}</div>
			<div class="p-4">
				<div class="flex gap-5 mb-3 text-xs text-secondary">
					<span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-[var(--green)]"></span>{stats.containers_running} {$t('dash.active').toLowerCase()}</span>
					<span class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-[var(--red)]"></span>{stats.containers_stopped} {$t('dash.stopped').toLowerCase()}</span>
				</div>
				<div class="flex h-2 rounded-full overflow-hidden bg-0">
					<div class="bg-[var(--green)] transition-all" style="width:{runPct}%"></div>
					<div class="bg-[var(--red)] transition-all" style="width:{100-runPct}%"></div>
				</div>
			</div>
		</div>
		{#if sys}
			<div class="bg-card border border-theme rounded-lg">
				<div class="px-4 py-3 border-b border-theme text-sm font-semibold text-primary">{$t('dash.system')}</div>
				<div class="p-4 grid grid-cols-2 gap-y-2.5 gap-x-4 text-xs">
					<span class="text-muted">Docker</span><span class="text-right font-medium text-primary">{sys.docker_version}</span>
					<span class="text-muted">CPU</span><span class="text-right font-medium text-primary">{sys.cpus} {$t('dash.cores')}</span>
					<span class="text-muted">RAM</span><span class="text-right font-medium text-primary">{sys.memory_display}</span>
					<span class="text-muted">Host</span><span class="text-right font-medium text-primary truncate">{sys.hostname}</span>
				</div>
			</div>
		{/if}
	</div>

	<!-- Containers table -->
	<div class="bg-card border border-theme rounded-lg overflow-hidden">
		<div class="px-4 py-3 border-b border-theme flex items-center justify-between">
			<span class="text-sm font-semibold text-primary">{$t('containers.title')} ({containers.length})</span>
			<a href="/containers" class="text-xs text-accent hover:text-accent-hover transition">{$t('dash.showAll')}</a>
		</div>
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead>
					<tr class="border-b border-theme">
						<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.name')}</th>
						<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">Image</th>
						<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
						<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">Ports</th>
					</tr>
				</thead>
				<tbody>
					{#each containers as c}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
							<td class="px-4 py-2.5 text-sm font-medium text-primary">{c.name}</td>
							<td class="px-4 py-2.5 text-xs text-secondary max-w-[150px] truncate">{c.image}</td>
							<td class="px-4 py-2.5"><Badge status={c.state} health={extractHealth(c.status)} /></td>
							<td class="px-4 py-2.5 text-xs text-secondary font-mono hidden md:table-cell">
								{c.ports.filter(p => p.public_port).map(p => `${p.public_port}:${p.private_port}`).join(', ') || '—'}
							</td>
						</tr>
					{:else}
						<tr><td colspan="4" class="text-center py-8 text-sm text-muted">{$t('containers.noContainers')}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>
{/if}
