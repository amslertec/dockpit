<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { truncateId, formatSize, formatDate } from '$lib/utils/format';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import { canManageDocker } from '$lib/stores/auth';
	import { t } from '$lib/i18n';
	import type { ImageInfo } from '$lib/api/types';

	let images = $state<ImageInfo[]>([]);
	let loading = $state(true);
	let showPull = $state(false);
	let pullName = $state('');
	let searchResults = $state<{name: string; description: string; is_official: boolean; star_count: number}[]>([]);
	let searchLoading = $state(false);
	let showSuggestions = $state(false);
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;
	let selectedIndex = $state(-1);
	let pullInputEl: HTMLInputElement | undefined = $state();
	let suggestionsStyle = $state('');
	let suggestionsEl: HTMLDivElement | undefined = $state();

	$effect(() => {
		if (showSuggestions && suggestionsEl && suggestionsEl.parentElement !== document.body) {
			document.body.appendChild(suggestionsEl);
		}
	});

	function positionSuggestions() {
		if (!pullInputEl) return;
		const rect = pullInputEl.getBoundingClientRect();
		suggestionsStyle = `position:fixed; left:${rect.left}px; top:${rect.bottom + 4}px; width:${rect.width}px; z-index:99999;`;
	}

	function onPullInput(val: string) {
		pullName = val;
		selectedIndex = -1;
		if (searchTimeout) clearTimeout(searchTimeout);
		if (val.length < 2 || val.includes(':')) {
			searchResults = [];
			showSuggestions = false;
			return;
		}
		searchLoading = true;
		showSuggestions = true;
		positionSuggestions();
		searchTimeout = setTimeout(async () => {
			try {
				const r = await api.get<{name: string; description: string; is_official: boolean; star_count: number}[]>(`/search/images?q=${encodeURIComponent(val)}`);
				if (r.success && r.data) {
					searchResults = r.data;
				}
			} catch {}
			searchLoading = false;
		}, 300);
	}

	function selectSuggestion(name: string) {
		pullName = name + ':latest';
		showSuggestions = false;
		searchResults = [];
	}

	function handlePullKeydown(e: KeyboardEvent) {
		if (!showSuggestions || searchResults.length === 0) return;
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = (selectedIndex + 1) % searchResults.length;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = (selectedIndex - 1 + searchResults.length) % searchResults.length;
		} else if (e.key === 'Enter' && selectedIndex >= 0) {
			e.preventDefault();
			selectSuggestion(searchResults[selectedIndex].name);
		} else if (e.key === 'Escape') {
			showSuggestions = false;
		}
	}
	let pruning = $state(false);
	let search = $state('');
	let page = $state(1);
	let perPage = $state(10);
	let filter = $state<'all' | 'used' | 'unused'>('all');
	let selected = $state<Set<string>>(new Set());
	let bulkRunning = $state(false);
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);

	let sortKey = $state<string>('tags');
	let sortAsc = $state(true);

	function toggleSort(key: string) {
		if (sortKey === key) { sortAsc = !sortAsc; }
		else { sortKey = key; sortAsc = false; }
	}

	function sortIndicator(key: string): string {
		if (sortKey !== key) return '';
		return sortAsc ? ' ▲' : ' ▼';
	}

	function toggleSelect(id: string) { const s = new Set(selected); if (s.has(id)) s.delete(id); else s.add(id); selected = s; }
	function toggleAll() { selected = allSelected ? new Set() : new Set(paged.filter(i => !i.in_use).map(i => i.id)); }

	async function bulkForceDelete() {
		if (selected.size === 0) return;
		confirmDlg = { message: $t('images.confirmForceDelete', { count: selected.size }), action: async () => {
			confirmDlg = null;
			bulkRunning = true;
			let ok = 0, fail = 0;
			for (const id of selected) {
				const r = await api.del<string>(`/env/${$selectedEnv}/images/${encodeURIComponent(id)}?force=true`);
				if (r.success) ok++; else fail++;
			}
			bulkRunning = false;
			if (ok > 0) toasts.success($t('images.bulkDeleted', { count: ok }));
			if (fail > 0) toasts.error($t('images.bulkFailed', { count: fail }));
			selected = new Set();
			load();
		}};
	}

	const filtered = $derived(
		images
			.filter(i => {
				if (filter === 'used') return i.in_use;
				if (filter === 'unused') return !i.in_use;
				return true;
			})
			.filter(i =>
				i.tags.some(t => t.toLowerCase().includes(search.toLowerCase())) ||
				i.id.toLowerCase().includes(search.toLowerCase())
			)
			.sort((a, b) => {
				if (sortKey === 'tags') {
					const av = (a.tags[0] || '').toLowerCase();
					const bv = (b.tags[0] || '').toLowerCase();
					return sortAsc ? av.localeCompare(bv) : bv.localeCompare(av);
				}
				const av = (a as any)[sortKey];
				const bv = (b as any)[sortKey];
				if (typeof av === 'number' && typeof bv === 'number') return sortAsc ? av - bv : bv - av;
				if (typeof av === 'boolean' && typeof bv === 'boolean') return sortAsc ? (av === bv ? 0 : av ? -1 : 1) : (av === bv ? 0 : av ? 1 : -1);
				return sortAsc ? String(av ?? '').localeCompare(String(bv ?? '')) : String(bv ?? '').localeCompare(String(av ?? ''));
			})
	);

	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));
	const allSelected = $derived(paged.length > 0 && paged.filter(i => !i.in_use).every(i => selected.has(i.id)));
	const someSelected = $derived(selected.size > 0);
	const usedCount = $derived(images.filter(i => i.in_use).length);
	const unusedCount = $derived(images.filter(i => !i.in_use).length);
	const totalSize = $derived(images.reduce((a, i) => a + i.size, 0));

	onMount(() => load());
	$effect(() => { $selectedEnv; load(); });
	$effect(() => { search; filter; page = 1; });

	async function load() {
		if (!$selectedEnv) return;
		loading = true;
		const r = await api.get<ImageInfo[]>(`/env/${$selectedEnv}/images`);
		if (r.success) images = r.data || [];
		loading = false;
	}

	async function pull(e: Event) {
		e.preventDefault();
		showPull = false;
		toasts.success($t('images.pulling'));
		const r = await api.post<string>(`/env/${$selectedEnv}/images/pull`, { image: pullName });
		if (r.success) { toasts.success($t('images.pulled')); pullName = ''; load(); }
		else toasts.error(r.error || $t('common.error'));
	}

	function remove(id: string, force = false) {
		confirmDlg = { message: force ? $t('images.confirmForceDeleteSingle') : $t('images.confirmDelete'), action: async () => {
			confirmDlg = null;
			// Force delete via the API - we pass force as query param
			const r = await api.del<string>(`/env/${$selectedEnv}/images/${encodeURIComponent(id)}${force ? '?force=true' : ''}`);
			if (r.success) { toasts.success($t('images.deleted')); load(); }
			else toasts.error(r.error || $t('common.error'));
		}};
	}

	function prune() {
		confirmDlg = { message: $t('images.pruneUnused') + '?', action: async () => {
			confirmDlg = null;
			pruning = true;
			const r = await api.post<string>(`/env/${$selectedEnv}/images/prune`, {});
			pruning = false;
			if (r.success) { toasts.success(r.data || $t('images.deleted')); load(); }
			else toasts.error(r.error || $t('common.error'));
		}};
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — Images</title></svelte:head>

<!-- Stats -->
{#if !loading}
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{images.length}</div>
			<div class="text-[11px] text-secondary">{$t('images.total')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{usedCount}</div>
			<div class="text-[11px] text-secondary">{$t('images.inUse')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-yellow">{unusedCount}</div>
			<div class="text-[11px] text-secondary">{$t('images.unused')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{formatSize(totalSize)}</div>
			<div class="text-[11px] text-secondary">{$t('images.storage')}</div>
		</div>
	</div>
{/if}

<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<div class="flex items-center gap-2">
			<h3 class="text-sm font-semibold text-primary">{$t('images.title')}</h3>
			<!-- Filter tabs -->
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
			{#if $canManageDocker}
			<Button variant="danger" size="sm" loading={pruning} onclick={prune}>
				{#if !pruning}
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/></svg>
				{/if}
				{$t('images.pruneUnused')}
			</Button>
			<Button variant="primary" size="sm" onclick={() => showPull = true}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
				{$t('images.pull')}
			</Button>
			{/if}
			<Button variant="success" size="sm" onclick={load} title={$t('common.refresh')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</Button>
		</div>
	</div>

	<!-- Bulk actions -->
	{#if someSelected}
		<div class="px-4 py-2 border-b border-theme bg-1 flex items-center gap-2 flex-wrap">
			<span class="text-xs text-secondary">{$t('common.selected', { count: selected.size })}</span>
			<Button variant="danger" size="sm" loading={bulkRunning} onclick={bulkForceDelete}>
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
				{$t('images.forceDelete')}
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
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('in_use')}>{$t('common.status')}{sortIndicator('in_use')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('tags')}>{$t('images.repoTag')}{sortIndicator('tags')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('id')}>ID{sortIndicator('id')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('size')}>{$t('images.size')}{sortIndicator('size')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('created')}>{$t('images.created')}{sortIndicator('created')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as img}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition {!img.in_use ? 'opacity-70' : ''} {selected.has(img.id) ? 'bg-accent-light' : ''}">
							<td class="w-10 px-4 py-3">
								{#if !img.in_use}
									<CustomCheckbox checked={selected.has(img.id)} onchange={() => toggleSelect(img.id)} size="sm" />
								{/if}
							</td>
							<td class="px-4 py-3">
								{#if img.in_use}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-green-light text-green">
										<span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('images.inUse')}
									</span>
								{:else}
									<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-yellow-light text-yellow">
										<span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('images.unused')}
									</span>
								{/if}
							</td>
							<td class="px-4 py-3 text-sm text-primary">{img.tags.length ? img.tags.join(', ') : '<none>'}</td>
							<td class="px-4 py-3 text-[11px] font-mono text-secondary">{truncateId(img.id)}</td>
							<td class="px-4 py-3 text-xs text-secondary">{formatSize(img.size)}</td>
							<td class="px-4 py-3 text-xs text-secondary hidden md:table-cell">{formatDate(img.created)}</td>
							<td class="px-4 py-3">
								<div class="flex gap-1">
									{#if img.in_use}
										<button disabled class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-muted opacity-30 cursor-not-allowed" title={$t('images.inUseCannotDelete')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
										</button>
									{:else}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('common.delete')} onclick={() => remove(img.id)}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
										</button>
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('images.forceDelete')} onclick={() => remove(img.id, true)}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
										</button>
									{/if}
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="7" class="text-center py-10 text-sm text-muted">{$t('images.noImages')}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
		<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
	{/if}
</div>

{#if showPull}
	<Modal title={$t('images.pullTitle')} onclose={() => { showPull = false; showSuggestions = false; }}>
		<form onsubmit={pull}>
			<div class="mb-4 relative">
				<label for="pin" class="block text-xs font-medium text-[var(--text-secondary)] mb-1.5">Image</label>
				<input
					bind:this={pullInputEl}
					id="pin"
					type="text"
					value={pullName}
					oninput={(e) => onPullInput((e.target as HTMLInputElement).value)}
					onkeydown={handlePullKeydown}
					onfocus={() => { if (searchResults.length > 0) { showSuggestions = true; positionSuggestions(); } }}
					placeholder="nginx:latest"
					required
					autocomplete="off"
					class="w-full bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-3 py-2.5 text-[16px] md:text-sm text-[var(--text)] placeholder:text-[var(--text-muted)] focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200"
				/>
				<p class="text-[10px] text-[var(--text-muted)] mt-1.5">Docker Hub search — type to find images</p>
			</div>
			<div class="flex justify-end gap-2">
				<Button variant="secondary" size="sm" type="button" onclick={() => { showPull = false; showSuggestions = false; }}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" type="submit">{$t('images.pull')}</Button>
			</div>
		</form>
	</Modal>
{/if}

{#if showSuggestions && (searchResults.length > 0 || searchLoading)}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div bind:this={suggestionsEl} class="bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] max-h-[280px] overflow-y-auto" style={suggestionsStyle}
		onclick={(e) => e.stopPropagation()}>
		{#if searchLoading && searchResults.length === 0}
			<div class="px-3 py-3 text-center">
				<div class="w-4 h-4 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin mx-auto"></div>
			</div>
		{/if}
		{#each searchResults as result, i}
			<div
				class="px-3 py-2.5 cursor-pointer transition-all duration-100 {i === selectedIndex ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'hover:bg-[var(--bg-hover)]'}"
				onclick={() => selectSuggestion(result.name)}
				onmouseenter={() => selectedIndex = i}
			>
				<div class="flex items-center gap-2">
					<span class="text-sm font-medium text-[var(--text)]">{result.name}</span>
					{#if result.is_official}
						<span class="text-[9px] px-1.5 py-0.5 rounded-full bg-[var(--accent-bg)] text-[var(--accent)] font-medium">Official</span>
					{/if}
					{#if result.star_count > 0}
						<span class="text-[10px] text-[var(--text-muted)] ml-auto">⭐ {result.star_count > 1000 ? Math.floor(result.star_count / 1000) + 'k' : result.star_count}</span>
					{/if}
				</div>
				{#if result.description}
					<div class="text-[11px] text-[var(--text-muted)] mt-0.5 truncate">{result.description}</div>
				{/if}
			</div>
		{/each}
	</div>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
