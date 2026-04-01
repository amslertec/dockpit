<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { canSeePage, canDoAction } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import { formatDateTime } from '$lib/utils/format';
	import type { UpdateCheckResult } from '$lib/api/types';

	interface CheckStatus { running: boolean; total_checked: number; total_outdated: number; last_check?: string; }

	$effect(() => {
		if (!$canSeePage('page.updates')) goto('/profile');
	});

	let results = $state<UpdateCheckResult[]>([]);
	let status = $state<CheckStatus>({ running: false, total_checked: 0, total_outdated: 0 });
	let loading = $state(true);
	let page = $state(1);
	let perPage = $state(15);
	let filter = $state<'all' | 'outdated' | 'current'>('all');
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);
	let pollInterval: ReturnType<typeof setInterval> | null = null;

	const filtered = $derived(results.filter(r => { if (filter === 'outdated') return r.outdated; if (filter === 'current') return !r.outdated; return true; }));
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));

	onMount(() => { loadAll(); startPolling(); });
	onDestroy(() => stopPolling());

	async function loadAll() {
		loading = true;
		const [rr, sr] = await Promise.all([
			api.get<UpdateCheckResult[]>('/updates/report'),
			api.get<CheckStatus>('/updates/status'),
		]);
		if (rr.success && rr.data) results = rr.data;
		if (sr.success && sr.data) status = sr.data;
		loading = false;
	}

	let lastResultCount = 0;

	function startPolling() {
		pollInterval = setInterval(async () => {
			const sr = await api.get<CheckStatus>('/updates/status');
			if (sr.success && sr.data) {
				const wasRunning = status.running;
				status = sr.data;

				// Live-refresh results while running (every 3 new results)
				if (sr.data.running && sr.data.total_checked > lastResultCount) {
					lastResultCount = sr.data.total_checked;
					const rr = await api.get<UpdateCheckResult[]>('/updates/report');
					if (rr.success && rr.data) results = rr.data;
				}

				// Check just completed
				if (wasRunning && !sr.data.running) {
					const rr = await api.get<UpdateCheckResult[]>('/updates/report');
					if (rr.success && rr.data) results = rr.data;
					lastResultCount = 0;
					toasts.success($t('updates.checkComplete', { checked: sr.data.total_checked, outdated: sr.data.total_outdated }));
				}
			}
		}, 3000);
	}

	function stopPolling() { if (pollInterval) clearInterval(pollInterval); }

	async function runCheck() {
		const r = await api.post<string>('/updates/check', {});
		if (r.success) {
			toasts.success($t('updates.checkStarted'));
			status = { ...status, running: true };
		} else {
			toasts.error(r.error || $t('common.error'));
		}
	}

	function clearReport() {
		confirmDlg = { message: $t('updates.confirmClear'), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>('/updates/report');
			if (r.success) { toasts.success($t('images.deleted')); results = []; status = { ...status, total_checked: 0, total_outdated: 0 }; }
			else toasts.error(r.error || $t('common.error'));
		}};
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — Update Monitor</title></svelte:head>

{#if !loading}
	<!-- Status Banner when running -->
	{#if status.running}
		<div class="bg-accent-light border border-[var(--accent)] rounded-lg p-4 mb-4 flex items-center gap-3">
			<div class="w-5 h-5 border-2 border-[var(--accent)]/30 border-t-[var(--accent)] rounded-full animate-spin shrink-0"></div>
			<div>
				<div class="text-sm font-medium text-accent">{$t('updates.running')}</div>
				<div class="text-xs text-secondary mt-0.5">
					{status.total_checked} {$t('updates.checked')}
					{#if status.total_outdated > 0} · {status.total_outdated} {$t('updates.updatesFound')}{/if}
					— {$t('updates.autoRefresh')}
				</div>
			</div>
		</div>
	{/if}

	<div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{status.total_checked}</div>
			<div class="text-[11px] text-secondary">{$t('updates.totalChecked')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{status.total_checked - status.total_outdated}</div>
			<div class="text-[11px] text-secondary">{$t('updates.current')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3 {status.total_outdated > 0 ? 'border-[var(--red)]' : ''}">
			<div class="text-xl font-bold {status.total_outdated > 0 ? 'text-red' : 'text-secondary'}">{status.total_outdated}</div>
			<div class="text-[11px] {status.total_outdated > 0 ? 'text-red' : 'text-secondary'}">{$t('updates.outdated')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3">
			<div class="text-xs text-secondary">{status.last_check ? $t('updates.lastCheck', { time: formatDateTime(status.last_check) }) : $t('updates.notChecked')}</div>
		</div>
	</div>
{/if}

<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<div class="flex items-center gap-2">
			<h3 class="text-sm font-semibold text-primary">{$t('updates.report')}</h3>
			<div class="flex items-center bg-1 rounded-md border border-theme ml-2">
				{#each [['all', $t('filter.all')], ['outdated', $t('filter.updates')], ['current', $t('filter.current')]] as [val, label]}
					<button onclick={() => { filter = val as any; page = 1; }}
						class="px-2.5 py-1 text-[11px] transition rounded-md
						{filter === val ? 'bg-accent text-white font-medium' : 'text-secondary hover:text-primary'}">
						{label}
					</button>
				{/each}
			</div>
		</div>
		<div class="flex items-center gap-2">
			{#if $canDoAction('action.container_recreate')}
			<Button variant="primary" size="sm" onclick={runCheck} disabled={status.running} loading={status.running}>
				{#if !status.running}<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>{/if}
				{status.running ? $t('updates.checkRunning') : $t('updates.checkNow')}
			</Button>
			{#if results.length > 0 && !status.running}
				<Button variant="danger" size="sm" onclick={clearReport}>{$t('updates.clearReport')}</Button>
			{/if}
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else if results.length === 0 && !status.running}
		<div class="text-center py-12">
			<svg class="w-12 h-12 mx-auto mb-3 opacity-20 text-secondary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
				<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
			</svg>
			<p class="text-sm text-secondary">{$t('updates.noResults')}</p>
			<p class="text-xs text-muted mt-1">{$t('updates.noResultsDesc')}</p>
		</div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containers.title')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">Image</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('updates.server')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('updates.totalChecked')}</th>
				</tr></thead>
				<tbody>
					{#each paged as r}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
							<td class="px-4 py-3">
								{#if r.outdated}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-red-light text-red">
										<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
										{$t('updates.update')}
									</span>
								{:else}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-green-light text-green">
										<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
										{$t('updates.current')}
									</span>
								{/if}
							</td>
							<td class="px-4 py-3 text-sm font-medium text-primary">{r.container_name}</td>
							<td class="px-4 py-3 text-xs text-secondary max-w-[200px] truncate">{r.image}</td>
							<td class="px-4 py-3 text-xs text-secondary">{r.server_name}</td>
							<td class="px-4 py-3 text-xs text-muted hidden md:table-cell">{formatDateTime(r.checked_at)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
	{/if}
</div>

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
