<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { truncateId } from '$lib/utils/format';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import { canManageDocker } from '$lib/stores/auth';
	import { t } from '$lib/i18n';
	import type { NetworkInfo } from '$lib/api/types';

	let networks = $state<NetworkInfo[]>([]);
	let loading = $state(true);
	let search = $state('');
	let page = $state(1);
	let perPage = $state(10);
	let filter = $state<'all' | 'used' | 'unused'>('all');
	let selected = $state<Set<string>>(new Set());
	let bulkRunning = $state(false);
	let pruning = $state(false);
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);

	const filtered = $derived(
		networks
			.filter(n => { if (filter === 'used') return n.in_use; if (filter === 'unused') return !n.in_use; return true; })
			.filter(n => n.name.toLowerCase().includes(search.toLowerCase()) || n.driver.toLowerCase().includes(search.toLowerCase()))
	);
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));
	const allSelected = $derived(paged.length > 0 && paged.filter(n => !n.in_use && !isSystem(n)).every(n => selected.has(n.id)));
	const someSelected = $derived(selected.size > 0);
	const usedCount = $derived(networks.filter(n => n.in_use).length);
	const unusedCount = $derived(networks.filter(n => !n.in_use).length);

	// System networks that should not be deleted
	function isSystem(n: NetworkInfo): boolean {
		return ['bridge', 'host', 'none'].includes(n.name);
	}

	onMount(() => load());
	$effect(() => { $selectedEnv; load(); });
	$effect(() => { search; filter; page = 1; });

	async function load() {
		if (!$selectedEnv) return;
		loading = true; selected = new Set();
		const r = await api.get<NetworkInfo[]>(`/env/${$selectedEnv}/networks`);
		if (r.success) networks = r.data || [];
		loading = false;
	}

	function toggleSelect(id: string) { const s = new Set(selected); if (s.has(id)) s.delete(id); else s.add(id); selected = s; }
	function toggleAll() { selected = allSelected ? new Set() : new Set(paged.filter(n => !n.in_use && !isSystem(n)).map(n => n.id)); }

	function remove(id: string, name: string) {
		confirmDlg = { message: $t('networks.confirmDelete', { name }), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/env/${$selectedEnv}/networks/${encodeURIComponent(id)}`);
			if (r.success) { toasts.success($t('networks.deleted')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function bulkDelete() {
		if (selected.size === 0) return;
		confirmDlg = { message: $t('networks.confirmBulkDelete', { count: selected.size }), action: async () => {
			confirmDlg = null;
			bulkRunning = true;
			let ok = 0, fail = 0;
			for (const id of selected) {
				const r = await api.del<string>(`/env/${$selectedEnv}/networks/${encodeURIComponent(id)}`);
				if (r.success) ok++; else fail++;
			}
			bulkRunning = false;
			if (ok > 0) toasts.success($t('networks.bulkDeleted', { count: ok }));
			if (fail > 0) toasts.error($t('networks.bulkFailed', { count: fail }));
			selected = new Set(); load();
		}};
	}

	function prune() {
		confirmDlg = { message: $t('networks.pruneUnused') + '?', action: async () => {
			confirmDlg = null;
			pruning = true;
			const r = await api.post<string>(`/env/${$selectedEnv}/networks/prune`, {});
			pruning = false;
			if (r.success) { toasts.success(r.data || $t('networks.deleted')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — {$t('networks.title')}</title></svelte:head>

{#if !loading}
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{networks.length}</div>
			<div class="text-[11px] text-secondary">{$t('networks.total')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{usedCount}</div>
			<div class="text-[11px] text-secondary">{$t('networks.inUse')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-yellow">{unusedCount}</div>
			<div class="text-[11px] text-secondary">{$t('networks.unused')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{networks.reduce((a, n) => a + n.containers_count, 0)}</div>
			<div class="text-[11px] text-secondary">{$t('networks.connectedContainers')}</div>
		</div>
	</div>
{/if}

<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<div class="flex items-center gap-2">
			<h3 class="text-sm font-semibold text-primary">{$t('networks.title')}</h3>
			<div class="flex items-center bg-1 rounded-md border border-theme ml-2">
				{#each [['all', $t('filter.all')], ['used', $t('filter.used')], ['unused', $t('filter.unused')]] as [val, label]}
					<button onclick={() => filter = val as any}
						class="px-2.5 py-1 text-[11px] transition rounded-md
						{filter === val ? 'bg-accent text-white font-medium' : 'text-secondary hover:text-primary'}">
						{label}
					</button>
				{/each}
			</div>
		</div>
		<div class="flex items-center gap-2 flex-wrap">
			<input bind:value={search} placeholder={$t('common.search')}
				class="bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs w-44 focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200" />
			<Button variant="danger" size="sm" loading={pruning} onclick={prune}>
				{#if !pruning}
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/></svg>
				{/if}
				{$t('networks.pruneUnused')}
			</Button>
			<button onclick={load} title={$t('common.refresh')}
				class="inline-flex items-center justify-center w-8 h-8 border border-theme text-secondary hover:text-primary hover:border-light rounded-[var(--radius-md)] transition">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</button>
		</div>
	</div>

	{#if someSelected && $canManageDocker}
		<div class="px-4 py-2 border-b border-theme bg-1 flex items-center gap-2 flex-wrap">
			<span class="text-xs text-secondary">{$t('common.selected', { count: selected.size })}</span>
			<Button variant="danger" size="sm" loading={bulkRunning} onclick={bulkDelete}>
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/></svg>
				{$t('common.delete')}
			</Button>
		</div>
	{/if}

	{#if loading}
		<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="w-10 px-4 py-2.5"><CustomCheckbox checked={allSelected} onchange={toggleAll} size="sm" /></th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.name')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('volumes.driver')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('networks.scope')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('containers.title')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden lg:table-cell">ID</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as n}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition {!n.in_use ? 'opacity-70' : ''} {selected.has(n.id) ? 'bg-accent-light' : ''}">
							<td class="w-10 px-4 py-3">
								{#if !n.in_use && !isSystem(n)}<CustomCheckbox checked={selected.has(n.id)} onchange={() => toggleSelect(n.id)} size="sm" />{/if}
							</td>
							<td class="px-4 py-3">
								{#if isSystem(n)}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-accent-light text-accent"><span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('networks.system')}</span>
								{:else if n.in_use}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-green-light text-green"><span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('networks.inUse')}</span>
								{:else}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-yellow-light text-yellow"><span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('networks.unused')}</span>
								{/if}
							</td>
							<td class="px-4 py-3 text-sm font-medium text-primary">{n.name}</td>
							<td class="px-4 py-3 text-xs text-secondary">{n.driver}</td>
							<td class="px-4 py-3 text-xs text-secondary">{n.scope}</td>
							<td class="px-4 py-3 text-xs text-secondary hidden md:table-cell">{n.containers_count}</td>
							<td class="px-4 py-3 text-[11px] font-mono text-muted hidden lg:table-cell">{truncateId(n.id)}</td>
							<td class="px-4 py-3">
								{#if n.in_use || isSystem(n)}
									<button disabled class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-muted opacity-30 cursor-not-allowed" title={isSystem(n) ? $t('networks.systemNetwork') : $t('networks.inUse')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
									</button>
								{:else}
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--red)] hover:border-[var(--red)] transition" title={$t('common.delete')} onclick={() => remove(n.id, n.name)}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
									</button>
								{/if}
							</td>
						</tr>
					{:else}
						<tr><td colspan="8" class="text-center py-10 text-sm text-muted">{$t('networks.noNetworks')}</td></tr>
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
