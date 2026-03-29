<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import { formatDateTime } from '$lib/utils/format';
	import type { ContainerHealth } from '$lib/api/types';

	let containers = $state<ContainerHealth[]>([]);
	let loading = $state(true);
	let page = $state(1);
	let perPage = $state(25);
	let refreshInterval: ReturnType<typeof setInterval> | undefined;
	let expandedRows = $state<Set<string>>(new Set());

	const withCheck = $derived(containers.filter(c => c.health_status !== 'none'));
	const healthy = $derived(containers.filter(c => c.health_status === 'healthy'));
	const unhealthy = $derived(containers.filter(c => c.health_status === 'unhealthy'));
	const starting = $derived(containers.filter(c => c.health_status === 'starting'));
	const noCheck = $derived(containers.filter(c => c.health_status === 'none'));

	const paged = $derived(
		perPage === 0 ? containers : containers.slice((page - 1) * perPage, page * perPage)
	);

	onMount(() => {
		loadHealth();
		refreshInterval = setInterval(() => loadHealth(), 15000);
	});

	onDestroy(() => {
		if (refreshInterval) clearInterval(refreshInterval);
	});

	$effect(() => { $selectedEnv; loadHealth(); });

	async function loadHealth() {
		if (!$selectedEnv) return;
		const r = await api.get<ContainerHealth[]>(`/env/${$selectedEnv}/health`);
		if (r.success && r.data) {
			containers = r.data;
		}
		loading = false;
	}

	function handlePageChange(p: number, pp: number) {
		page = p;
		perPage = pp;
	}

	function toggleRow(id: string) {
		const next = new Set(expandedRows);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		expandedRows = next;
	}

	function statusBadgeClass(status: string): string {
		switch (status) {
			case 'healthy': return 'bg-[var(--green-bg)] text-[var(--green)]';
			case 'unhealthy': return 'bg-[var(--red-bg)] text-[var(--red)]';
			case 'starting': return 'bg-[var(--yellow-bg)] text-[var(--yellow)]';
			default: return 'bg-[var(--bg-hover)] text-[var(--text-muted)]';
		}
	}

	function statusLabel(status: string): string {
		switch (status) {
			case 'healthy': return $t('health.healthy');
			case 'unhealthy': return $t('health.unhealthy');
			case 'starting': return $t('health.starting');
			default: return '—';
		}
	}

	function truncateOutput(output: string, max = 80): string {
		if (!output) return '-';
		return output.length > max ? output.slice(0, max) + '...' : output;
	}

	function truncateName(name: string): string {
		if (!name) return '-';
		const clean = name.startsWith('/') ? name.slice(1) : name;
		return clean.length > 28 ? clean.slice(0, 28) + '...' : clean;
	}
</script>

<div class="space-y-4">
	<!-- Header -->
	<div class="flex items-center justify-between flex-wrap gap-3">
		<div>
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('health.title')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{containers.length} containers</p>
		</div>
		<div class="flex items-center gap-2">
			<div class="flex items-center gap-1.5 text-[11px] text-[var(--text-muted)]">
				<span class="relative flex h-2 w-2">
					<span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-[var(--green)] opacity-75"></span>
					<span class="relative inline-flex rounded-full h-2 w-2 bg-[var(--green)]"></span>
				</span>
				15s auto-refresh
			</div>
		</div>
	</div>

	<!-- Summary Cards -->
	<div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3">
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<div class="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.totalChecked')}</div>
			<div class="text-2xl font-bold text-[var(--text)] mt-1">{withCheck.length}</div>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<div class="text-[11px] font-semibold uppercase tracking-wider text-[var(--green)]">{$t('health.healthy')}</div>
			<div class="text-2xl font-bold text-[var(--green)] mt-1">{healthy.length}</div>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<div class="text-[11px] font-semibold uppercase tracking-wider text-[var(--red)]">{$t('health.unhealthy')}</div>
			<div class="text-2xl font-bold text-[var(--red)] mt-1">{unhealthy.length}</div>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<div class="text-[11px] font-semibold uppercase tracking-wider text-[var(--yellow)]">{$t('health.starting')}</div>
			<div class="text-2xl font-bold text-[var(--yellow)] mt-1">{starting.length}</div>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<div class="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.noCheck')}</div>
			<div class="text-2xl font-bold text-[var(--text-muted)] mt-1">{noCheck.length}</div>
		</div>
	</div>

	<!-- Table -->
	<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] overflow-hidden">
		{#if loading}
			<div class="flex items-center justify-center py-16">
				<div class="w-6 h-6 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
			</div>
		{:else if containers.length === 0}
			<div class="flex flex-col items-center justify-center py-16 text-center">
				<svg class="w-10 h-10 text-[var(--text-muted)] mb-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/></svg>
				<p class="text-sm font-medium text-[var(--text-secondary)]">{$t('health.noContainers')}</p>
				<p class="text-xs text-[var(--text-muted)] mt-1 max-w-[300px]">{$t('health.noContainersDesc')}</p>
			</div>
		{:else}
			<div class="overflow-x-auto">
				<table class="w-full text-sm">
					<thead>
						<tr class="border-b border-[var(--border)]">
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('events.container')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">Image</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('common.status')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.checkCmd')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.interval')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.failStreak')}</th>
						</tr>
					</thead>
					<tbody>
						{#each paged as container (container.id)}
							<tr
								class="border-b border-[var(--border)] last:border-0 hover:bg-[var(--bg-hover)] transition-colors duration-150 cursor-pointer"
								onclick={() => toggleRow(container.id)}
							>
								<td class="px-4 py-3 text-xs text-[var(--text)] font-medium" title={container.name}>{truncateName(container.name)}</td>
								<td class="px-4 py-3 text-xs text-[var(--text-secondary)] font-mono">{container.image}</td>
								<td class="px-4 py-3">
									<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium {statusBadgeClass(container.health_status)}">
										{statusLabel(container.health_status)}
									</span>
								</td>
								<td class="px-4 py-3 text-xs text-[var(--text-secondary)] font-mono max-w-[200px] truncate" title={container.health_check || '-'}>{container.health_check || '-'}</td>
								<td class="px-4 py-3 text-xs text-[var(--text-secondary)]">{container.health_interval || '-'}</td>
								<td class="px-4 py-3 text-xs">
									{#if container.failing_streak > 0}
										<span class="text-[var(--red)] font-medium">{container.failing_streak}</span>
									{:else}
										<span class="text-[var(--text-muted)]">0</span>
									{/if}
								</td>
							</tr>
							{#if expandedRows.has(container.id) && container.health_log && container.health_log.length > 0}
								<tr class="bg-[var(--bg-hover)]">
									<td colspan="6" class="px-4 py-3">
										<div class="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)] mb-2">{$t('health.log')} ({Math.min(container.health_log.length, 5)})</div>
										<table class="w-full text-xs">
											<thead>
												<tr class="border-b border-[var(--border)]">
													<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('events.time')}</th>
													<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.exitCode')}</th>
													<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('health.output')}</th>
												</tr>
											</thead>
											<tbody>
												{#each container.health_log.slice(0, 5) as log}
													<tr class="border-b border-[var(--border)] last:border-0">
														<td class="px-3 py-2 text-[var(--text-muted)] font-mono whitespace-nowrap">{formatDateTime(log.start)}</td>
														<td class="px-3 py-2">
															<span class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium {log.exit_code === 0 ? 'bg-[var(--green-bg)] text-[var(--green)]' : 'bg-[var(--red-bg)] text-[var(--red)]'}">
																{log.exit_code}
															</span>
														</td>
														<td class="px-3 py-2 text-[var(--text-secondary)] max-w-[400px] truncate" title={log.output}>{truncateOutput(log.output)}</td>
													</tr>
												{/each}
											</tbody>
										</table>
									</td>
								</tr>
							{/if}
						{/each}
					</tbody>
				</table>
			</div>

			<Pagination total={containers.length} {page} {perPage} onchange={handlePageChange} />
		{/if}
	</div>
</div>
