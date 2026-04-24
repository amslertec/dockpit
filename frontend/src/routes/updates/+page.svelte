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
	import { initResizableColumns } from '$lib/utils/resizable-columns';
	import type { UpdateCheckResult } from '$lib/api/types';

	interface CheckStatus { running: boolean; total_checked: number; total_outdated: number; last_check?: string; }
	interface RecreateStep { text: string; status: 'pending' | 'running' | 'done' | 'error'; detail?: string; }
	interface RecreateModal {
		name: string;
		image: string;
		steps: RecreateStep[];
		output: string;
		done: boolean;
		progress?: { current: number; total: number };
	}

	$effect(() => {
		if (!$canSeePage('page.updates')) goto('/profile');
	});

	let tableEl: HTMLTableElement | undefined = $state();
	let results = $state<UpdateCheckResult[]>([]);
	let status = $state<CheckStatus>({ running: false, total_checked: 0, total_outdated: 0 });
	let loading = $state(true);
	let page = $state(1);
	let perPage = $state(15);
	let filter = $state<'all' | 'outdated' | 'current'>('all');
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);
	let recreateModal = $state<RecreateModal | null>(null);
	let updatingIds = $state(new Set<number>());
	let pollInterval: ReturnType<typeof setInterval> | null = null;

	const filtered = $derived(results.filter(r => { if (filter === 'outdated') return r.outdated; if (filter === 'current') return !r.outdated; return true; }));
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));
	const outdatedRows = $derived(results.filter(r => r.outdated));

	$effect(() => { if (tableEl && !loading && results.length > 0) initResizableColumns(tableEl); });

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

	function setStep(idx: number, step: RecreateStep['status'], detail?: string) {
		if (!recreateModal) return;
		recreateModal.steps[idx].status = step;
		if (detail) recreateModal.steps[idx].detail = detail;
		recreateModal = { ...recreateModal };
	}

	/** Recreate a single container from the report. Returns true on success. */
	async function recreateOne(row: UpdateCheckResult): Promise<boolean> {
		const r = await api.post<string>(`/env/${row.env_id}/containers/${encodeURIComponent(row.container_name)}/recreate`, {});
		if (r.success) {
			// Optimistically mark row as current (remove from outdated list)
			results = results.map(x => x.id === row.id ? { ...x, outdated: false } : x);
			status = { ...status, total_outdated: Math.max(0, status.total_outdated - 1) };
			return true;
		}
		if (!recreateModal) return false;
		recreateModal.output = (recreateModal.output ? recreateModal.output + '\n\n' : '') + `${row.container_name}: ${r.error || $t('common.error')}`;
		recreateModal = { ...recreateModal };
		return false;
	}

	function confirmUpdate(row: UpdateCheckResult) {
		confirmDlg = {
			message: $t('updates.confirmUpdate', { name: row.container_name }),
			action: async () => {
				confirmDlg = null;
				await doSingleUpdate(row);
			}
		};
	}

	async function doSingleUpdate(row: UpdateCheckResult) {
		if (updatingIds.has(row.id)) return;
		updatingIds = new Set([...updatingIds, row.id]);

		recreateModal = {
			name: row.container_name,
			image: row.image,
			output: '',
			done: false,
			steps: [
				{ text: $t('containers.pullImage'), status: 'running' },
				{ text: $t('containers.stopRemove'), status: 'pending' },
				{ text: $t('containers.createStart'), status: 'pending' },
			]
		};

		// Simulated step progression while waiting for the API
		const t1 = setTimeout(() => { if (recreateModal && !recreateModal.done) { setStep(0, 'done'); setStep(1, 'running'); } }, 3000);
		const t2 = setTimeout(() => { if (recreateModal && !recreateModal.done) { setStep(1, 'done'); setStep(2, 'running'); } }, 6000);

		const ok = await recreateOne(row);
		clearTimeout(t1); clearTimeout(t2);
		if (!recreateModal) {
			updatingIds = new Set([...updatingIds].filter(i => i !== row.id));
			return;
		}
		if (ok) {
			recreateModal.steps.forEach(s => { s.status = 'done'; });
			recreateModal.output = $t('updates.updateSuccess', { name: row.container_name });
		} else {
			const failIdx = recreateModal.steps.findIndex(s => s.status === 'running');
			if (failIdx >= 0) recreateModal.steps[failIdx].status = 'error';
		}
		recreateModal.done = true;
		recreateModal = { ...recreateModal };
		updatingIds = new Set([...updatingIds].filter(i => i !== row.id));
	}

	function confirmUpdateAll() {
		const n = outdatedRows.length;
		if (n === 0) return;
		confirmDlg = {
			message: $t('updates.confirmUpdateAll', { count: n }),
			action: async () => {
				confirmDlg = null;
				await doUpdateAll();
			}
		};
	}

	async function doUpdateAll() {
		const rows = [...outdatedRows];
		if (rows.length === 0) return;

		recreateModal = {
			name: $t('updates.updateAllTitle', { count: rows.length }),
			image: '',
			output: '',
			done: false,
			progress: { current: 0, total: rows.length },
			steps: rows.map(r => ({ text: `${r.container_name} — ${r.image}`, status: 'pending' as const }))
		};

		let ok = 0, fail = 0;
		for (let i = 0; i < rows.length; i++) {
			if (!recreateModal) break;
			updatingIds = new Set([...updatingIds, rows[i].id]);
			recreateModal.steps[i].status = 'running';
			recreateModal.progress = { current: i + 1, total: rows.length };
			recreateModal = { ...recreateModal };

			const success = await recreateOne(rows[i]);
			if (!recreateModal) { updatingIds = new Set([...updatingIds].filter(x => x !== rows[i].id)); break; }
			recreateModal.steps[i].status = success ? 'done' : 'error';
			updatingIds = new Set([...updatingIds].filter(x => x !== rows[i].id));
			if (success) ok++; else fail++;
			recreateModal = { ...recreateModal };
		}

		if (recreateModal) {
			recreateModal.done = true;
			recreateModal.output = (recreateModal.output ? recreateModal.output + '\n\n' : '')
				+ $t('updates.updateAllDone', { ok, fail });
			recreateModal = { ...recreateModal };
		}
		updatingIds = new Set();
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
			{#if outdatedRows.length > 0 && !status.running && !recreateModal}
				<Button variant="success" size="sm" onclick={confirmUpdateAll} title={$t('updates.updateAllTitle', { count: outdatedRows.length })}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15A9 9 0 1 1 5.64 5.64L23 10"/></svg>
					<span class="ml-1">{$t('updates.updateAll', { count: outdatedRows.length })}</span>
				</Button>
			{/if}
			<Button variant="primary" size="sm" onclick={runCheck} disabled={status.running} loading={status.running} title={status.running ? $t('updates.checkRunning') : $t('updates.checkNow')}>
				{#if !status.running}<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>{/if}
			</Button>
			{#if results.length > 0 && !status.running}
				<Button variant="danger" size="sm" onclick={clearReport} title={$t('updates.clearReport')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/></svg>
				</Button>
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
			<table bind:this={tableEl} class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('containers.title')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">Image</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('updates.server')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('updates.totalChecked')}</th>
					{#if $canDoAction('action.container_recreate')}
					<th class="text-right px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold w-24">{$t('common.actions')}</th>
					{/if}
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
							{#if $canDoAction('action.container_recreate')}
							<td class="px-4 py-3 text-right">
								{#if r.outdated}
									<button
										onclick={() => confirmUpdate(r)}
										disabled={updatingIds.has(r.id) || !!recreateModal}
										title={$t('updates.updateNow')}
										class="inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-[11px] font-medium bg-accent text-white hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition">
										{#if updatingIds.has(r.id)}
											<div class="w-3 h-3 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
										{:else}
											<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
										{/if}
										<span>{$t('updates.update')}</span>
									</button>
								{/if}
							</td>
							{/if}
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

{#if recreateModal}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[1000] p-4"
		onclick={(e) => { if (e.target === e.currentTarget && recreateModal?.done) recreateModal = null; }}>
		<div class="bg-card border border-theme rounded-xl w-full max-w-xl shadow-2xl flex flex-col max-h-[85vh]">
			<!-- Header -->
			<div class="flex items-center justify-between px-6 py-4 border-b border-theme shrink-0">
				<div class="min-w-0">
					<h3 class="text-base font-semibold text-primary">{$t('containers.recreateTitle')}</h3>
					<p class="text-xs text-secondary mt-0.5 truncate">
						{recreateModal.name}{#if recreateModal.image} · {recreateModal.image}{/if}
					</p>
					{#if recreateModal.progress}
						<p class="text-[11px] text-muted mt-0.5">
							{recreateModal.progress.current} / {recreateModal.progress.total}
						</p>
					{/if}
				</div>
				{#if recreateModal.done}
					<button class="w-8 h-8 flex items-center justify-center rounded-md border border-theme text-secondary hover:text-primary hover:border-light transition"
						onclick={() => recreateModal = null}>
						<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					</button>
				{/if}
			</div>

			<!-- Steps -->
			<div class="px-6 py-5 space-y-4 overflow-y-auto">
				{#each recreateModal.steps as step, i}
					<div class="flex items-start gap-3">
						<div class="w-6 h-6 shrink-0 flex items-center justify-center mt-0.5">
							{#if step.status === 'running'}
								<div class="w-5 h-5 border-2 border-[var(--accent)]/30 border-t-[var(--accent)] rounded-full animate-spin"></div>
							{:else if step.status === 'done'}
								<div class="w-5 h-5 rounded-full bg-[var(--green)] flex items-center justify-center">
									<svg class="w-3 h-3 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
								</div>
							{:else if step.status === 'error'}
								<div class="w-5 h-5 rounded-full bg-[var(--red)] flex items-center justify-center">
									<svg class="w-3 h-3 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
								</div>
							{:else}
								<div class="w-5 h-5 rounded-full border-2 border-theme"></div>
							{/if}
						</div>
						<div class="flex-1 min-w-0">
							<p class="text-sm {step.status === 'running' ? 'text-primary font-medium' : step.status === 'done' ? 'text-secondary' : step.status === 'error' ? 'text-red' : 'text-muted'}">
								{step.text}
							</p>
							{#if step.detail}
								<p class="text-[11px] text-muted mt-1 font-mono break-all">{step.detail}</p>
							{/if}
						</div>
					</div>
				{/each}
			</div>

			<!-- Output -->
			{#if recreateModal.output && recreateModal.done}
				<div class="border-t border-theme">
					<details class="group" open={recreateModal.output.includes('updates.')}>
						<summary class="px-6 py-3 text-xs text-secondary cursor-pointer hover:text-primary transition flex items-center gap-2">
							<svg class="w-3 h-3 transition group-open:rotate-90" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
							{$t('containers.fullOutput')}
						</summary>
						<div class="px-6 pb-4">
							<div class="bg-0 border border-theme rounded-lg p-4 font-mono text-[11px] leading-[1.8] text-secondary max-h-[200px] overflow-y-auto whitespace-pre-wrap break-words">
								{recreateModal.output}
							</div>
						</div>
					</details>
				</div>
			{/if}

			<!-- Footer -->
			{#if recreateModal.done}
				<div class="px-6 py-4 border-t border-theme flex justify-end shrink-0">
					<Button variant="primary" onclick={() => recreateModal = null}>{$t('common.close')}</Button>
				</div>
			{/if}
		</div>
	</div>
{/if}
