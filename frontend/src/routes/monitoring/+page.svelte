<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { canSeePage } from '$lib/stores/auth';
	import { statsStore, currentStats } from '$lib/stores/stats';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import type { ContainerStats } from '$lib/api/types';

	let search = $state('');
	let sortKey = $state<keyof ContainerStats>('cpu_percent');
	let sortAsc = $state(false);
	let page = $state(1);
	let perPage = $state(15);

	function formatBytes(b: number): string {
		if (b < 1024) return b + ' B';
		if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
		if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
		return (b / 1073741824).toFixed(1) + ' GB';
	}

	function barColor(pct: number): string {
		if (pct < 50) return 'var(--green)';
		if (pct < 80) return 'var(--yellow)';
		return 'var(--red)';
	}

	function toggleSort(key: keyof ContainerStats) {
		if (sortKey === key) { sortAsc = !sortAsc; }
		else { sortKey = key; sortAsc = false; }
	}

	const filtered = $derived(
		($currentStats || [])
			.filter((c: ContainerStats) => !search || c.name.toLowerCase().includes(search.toLowerCase()))
			.sort((a: ContainerStats, b: ContainerStats) => {
				const av = a[sortKey];
				const bv = b[sortKey];
				if (typeof av === 'number' && typeof bv === 'number') {
					return sortAsc ? av - bv : bv - av;
				}
				return sortAsc
					? String(av).localeCompare(String(bv))
					: String(bv).localeCompare(String(av));
			})
	);

	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));

	$effect(() => { search; page = 1; });

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }

	const totalCpu = $derived(($currentStats || []).reduce((s: number, c: ContainerStats) => s + c.cpu_percent, 0));
	const totalMemUsage = $derived(($currentStats || []).reduce((s: number, c: ContainerStats) => s + c.memory_usage, 0));
	const totalMemLimit = $derived(($currentStats || []).reduce((s: number, c: ContainerStats) => s + c.memory_limit, 0));
	const totalMemPct = $derived(totalMemLimit > 0 ? (totalMemUsage / totalMemLimit) * 100 : 0);
	const totalRx = $derived(($currentStats || []).reduce((s: number, c: ContainerStats) => s + c.network_rx, 0));
	const totalTx = $derived(($currentStats || []).reduce((s: number, c: ContainerStats) => s + c.network_tx, 0));
	const containerCount = $derived(($currentStats || []).length);

	const hasData = $derived($currentStats && $currentStats.length > 0);

	$effect(() => {
		if (!$canSeePage('page.monitoring')) goto('/profile');
	});

	onMount(() => {
		if ($selectedEnv) statsStore.connect($selectedEnv);
	});

	onDestroy(() => {
		statsStore.disconnect();
	});

	$effect(() => {
		const envId = $selectedEnv;
		if (envId) statsStore.connect(envId);
	});

	function sortIndicator(key: keyof ContainerStats): string {
		if (sortKey !== key) return '';
		return sortAsc ? ' \u25B2' : ' \u25BC';
	}
</script>

<svelte:head>
	<title>DockPit — {$t('monitoring.title')}</title>
</svelte:head>

<div class="space-y-5">
	<!-- Summary Cards -->
	<div class="grid grid-cols-2 md:grid-cols-4 gap-4">
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-4">
			<div class="text-xs text-[var(--text-muted)] mb-1">{$t('monitoring.cpuUsage')}</div>
			<div class="text-2xl font-bold" style="color: {barColor(totalCpu)}">{totalCpu.toFixed(1)}%</div>
		</div>
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-4">
			<div class="text-xs text-[var(--text-muted)] mb-1">{$t('monitoring.memUsage')}</div>
			<div class="text-2xl font-bold" style="color: {barColor(totalMemPct)}">{totalMemPct.toFixed(1)}%</div>
			<div class="text-xs text-[var(--text-muted)]">{formatBytes(totalMemUsage)} / {formatBytes(totalMemLimit)}</div>
		</div>
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-4">
			<div class="text-xs text-[var(--text-muted)] mb-1">{$t('monitoring.netIO')}</div>
			<div class="text-lg font-bold text-[var(--text)]">
				<span class="text-[var(--green)]">{$t('monitoring.rx')}: {formatBytes(totalRx)}</span>
			</div>
			<div class="text-sm text-[var(--text-secondary)]">{$t('monitoring.tx')}: {formatBytes(totalTx)}</div>
		</div>
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-4">
			<div class="text-xs text-[var(--text-muted)] mb-1">{$t('monitoring.runningContainers')}</div>
			<div class="text-2xl font-bold text-[var(--accent)]">{containerCount}</div>
		</div>
	</div>

	<!-- Search -->
	<div class="flex items-center gap-3">
		<input
			type="text"
			bind:value={search}
			placeholder={$t('common.search')}
			class="px-3 py-2 text-sm rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-2)] text-[var(--text)] placeholder:text-[var(--text-muted)] w-full max-w-xs focus:outline-none focus:border-[var(--accent)]"
		/>
	</div>

	<!-- Table -->
	{#if !$statsStore}
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-8 text-center">
			<div class="flex flex-col items-center gap-3">
				<div class="w-6 h-6 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
				<p class="text-[var(--text-muted)] text-sm">{$t('monitoring.noData')}</p>
			</div>
		</div>
	{:else if !hasData}
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-8 text-center">
			<p class="text-[var(--text-muted)] text-sm">{$t('monitoring.noContainers')}</p>
		</div>
	{:else}
		<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] overflow-hidden">
			<div class="overflow-x-auto">
				<table class="w-full text-sm">
					<thead>
						<tr class="border-b border-[var(--border)] text-left text-[var(--text-muted)]">
							<th class="px-4 py-3 font-medium cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('name')}>
								{$t('monitoring.container')}{sortIndicator('name')}
							</th>
							<th class="px-4 py-3 font-medium cursor-pointer hover:text-[var(--text)] w-20 text-right" onclick={() => toggleSort('cpu_percent')}>
								{$t('monitoring.cpu')}%{sortIndicator('cpu_percent')}
							</th>
							<th class="px-4 py-3 font-medium w-32">
								{$t('monitoring.cpu')}
							</th>
							<th class="px-4 py-3 font-medium cursor-pointer hover:text-[var(--text)] text-right" onclick={() => toggleSort('memory_usage')}>
								{$t('monitoring.memory')}{sortIndicator('memory_usage')}
							</th>
							<th class="px-4 py-3 font-medium cursor-pointer hover:text-[var(--text)] w-16 text-right" onclick={() => toggleSort('memory_percent')}>
								%{sortIndicator('memory_percent')}
							</th>
							<th class="px-4 py-3 font-medium w-32">
								{$t('monitoring.memory')}
							</th>
							<th class="px-4 py-3 font-medium cursor-pointer hover:text-[var(--text)] text-right" onclick={() => toggleSort('network_rx')}>
								{$t('monitoring.rx')}{sortIndicator('network_rx')}
							</th>
							<th class="px-4 py-3 font-medium cursor-pointer hover:text-[var(--text)] text-right" onclick={() => toggleSort('network_tx')}>
								{$t('monitoring.tx')}{sortIndicator('network_tx')}
							</th>
						</tr>
					</thead>
					<tbody>
						{#each paged as c (c.id)}
							<tr class="border-b border-[var(--border)] last:border-0 hover:bg-[var(--bg-hover)] transition-colors">
								<td class="px-4 py-3 font-medium text-[var(--text)]">{c.name}</td>
								<td class="px-4 py-3 text-right tabular-nums" style="color: {barColor(c.cpu_percent)}">{c.cpu_percent.toFixed(1)}%</td>
								<td class="px-4 py-3">
									<div class="h-2 rounded-full bg-[var(--bg-2)] overflow-hidden">
										<div class="h-full rounded-full transition-all duration-500" style="width: {Math.min(c.cpu_percent, 100)}%; background: {barColor(c.cpu_percent)}"></div>
									</div>
								</td>
								<td class="px-4 py-3 text-right text-[var(--text-secondary)] tabular-nums">{formatBytes(c.memory_usage)}</td>
								<td class="px-4 py-3 text-right tabular-nums" style="color: {barColor(c.memory_percent)}">{c.memory_percent.toFixed(1)}%</td>
								<td class="px-4 py-3">
									<div class="h-2 rounded-full bg-[var(--bg-2)] overflow-hidden">
										<div class="h-full rounded-full transition-all duration-500" style="width: {Math.min(c.memory_percent, 100)}%; background: {barColor(c.memory_percent)}"></div>
									</div>
								</td>
								<td class="px-4 py-3 text-right text-[var(--text-secondary)] tabular-nums">{formatBytes(c.network_rx)}</td>
								<td class="px-4 py-3 text-right text-[var(--text-secondary)] tabular-nums">{formatBytes(c.network_tx)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
		</div>
	{/if}
</div>
