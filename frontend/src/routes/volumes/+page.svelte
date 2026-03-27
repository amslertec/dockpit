<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import { canManageDocker } from '$lib/stores/auth';
	import { t } from '$lib/i18n';
	import type { VolumeInfo } from '$lib/api/types';

	let volumes = $state<VolumeInfo[]>([]);
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
		volumes
			.filter(v => { if (filter === 'used') return v.in_use; if (filter === 'unused') return !v.in_use; return true; })
			.filter(v => v.name.toLowerCase().includes(search.toLowerCase()))
	);
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));
	const allSelected = $derived(paged.length > 0 && paged.filter(v => !v.in_use).every(v => selected.has(v.name)));
	const someSelected = $derived(selected.size > 0);
	const usedCount = $derived(volumes.filter(v => v.in_use).length);
	const unusedCount = $derived(volumes.filter(v => !v.in_use).length);

	onMount(() => load());
	$effect(() => { $selectedEnv; load(); });
	$effect(() => { search; filter; page = 1; });

	async function load() {
		if (!$selectedEnv) return;
		loading = true; selected = new Set();
		const r = await api.get<VolumeInfo[]>(`/env/${$selectedEnv}/volumes`);
		if (r.success) volumes = r.data || [];
		loading = false;
	}

	function toggleSelect(name: string) { const s = new Set(selected); if (s.has(name)) s.delete(name); else s.add(name); selected = s; }
	function toggleAll() { selected = allSelected ? new Set() : new Set(paged.filter(v => !v.in_use).map(v => v.name)); }

	function remove(name: string, force = false) {
		confirmDlg = { message: force ? $t('volumes.confirmForceDelete', { name }) : $t('volumes.confirmDelete', { name }), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/env/${$selectedEnv}/volumes/${encodeURIComponent(name)}${force ? '?force=true' : ''}`);
			if (r.success) { toasts.success($t('volumes.deleted')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function bulkForceDelete() {
		if (selected.size === 0) return;
		confirmDlg = { message: $t('volumes.confirmBulkDelete', { count: selected.size }), action: async () => {
			confirmDlg = null;
			bulkRunning = true;
			let ok = 0, fail = 0;
			for (const name of selected) {
				const r = await api.del<string>(`/env/${$selectedEnv}/volumes/${encodeURIComponent(name)}?force=true`);
				if (r.success) ok++; else fail++;
			}
			bulkRunning = false;
			if (ok > 0) toasts.success($t('volumes.bulkDeleted', { count: ok }));
			if (fail > 0) toasts.error($t('volumes.bulkFailed', { count: fail }));
			selected = new Set(); load();
		}};
	}

	function prune() {
		confirmDlg = { message: $t('volumes.pruneUnused') + '?', action: async () => {
			confirmDlg = null;
			pruning = true;
			const r = await api.post<string>(`/env/${$selectedEnv}/volumes/prune`, {});
			pruning = false;
			if (r.success) { toasts.success(r.data || $t('volumes.deleted')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — Volumes</title></svelte:head>

{#if !loading}
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{volumes.length}</div>
			<div class="text-[11px] text-secondary">{$t('volumes.total')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{usedCount}</div>
			<div class="text-[11px] text-secondary">{$t('volumes.inUse')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-yellow">{unusedCount}</div>
			<div class="text-[11px] text-secondary">{$t('volumes.unused')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{volumes.filter(v => v.driver === 'local').length}</div>
			<div class="text-[11px] text-secondary">{$t('volumes.local')}</div>
		</div>
	</div>
{/if}

<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<div class="flex items-center gap-2">
			<h3 class="text-sm font-semibold text-primary">{$t('volumes.title')}</h3>
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
				{$t('volumes.pruneUnused')}
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
			<Button variant="danger" size="sm" loading={bulkRunning} onclick={bulkForceDelete}>
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
				Force Delete
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
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('volumes.mountpoint')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as v}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition {!v.in_use ? 'opacity-70' : ''} {selected.has(v.name) ? 'bg-accent-light' : ''}">
							<td class="w-10 px-4 py-3">
								{#if !v.in_use}<CustomCheckbox checked={selected.has(v.name)} onchange={() => toggleSelect(v.name)} size="sm" />{/if}
							</td>
							<td class="px-4 py-3">
								{#if v.in_use}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-green-light text-green"><span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('volumes.inUse')}</span>
								{:else}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-yellow-light text-yellow"><span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('volumes.unused')}</span>
								{/if}
							</td>
							<td class="px-4 py-3 text-sm font-medium text-primary max-w-[250px] truncate">{v.name}</td>
							<td class="px-4 py-3 text-xs text-secondary">{v.driver}</td>
							<td class="px-4 py-3 text-[11px] font-mono text-muted max-w-[250px] truncate hidden md:table-cell">{v.mountpoint}</td>
							<td class="px-4 py-3">
								<div class="flex gap-1">
									{#if v.in_use}
										<button disabled class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-muted opacity-30 cursor-not-allowed" title={$t('volumes.inUse')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
										</button>
									{:else}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--red)] hover:border-[var(--red)] transition" title={$t('common.delete')} onclick={() => remove(v.name)}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
										</button>
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-[var(--red)]/30 text-red hover:bg-red-light transition" title={$t('images.forceDelete')} onclick={() => remove(v.name, true)}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
										</button>
									{/if}
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="6" class="text-center py-10 text-sm text-muted">{$t('volumes.noVolumes')}</td></tr>
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
