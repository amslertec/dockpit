<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import Modal from '$lib/components/ui/Modal.svelte';
	import { toasts } from '$lib/stores/toast';
	import { formatPorts, truncateId, formatDate, formatDateTime, extractHealth } from '$lib/utils/format';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import { canManageDocker, canEditContainers } from '$lib/stores/auth';
	import { t } from '$lib/i18n';
	import { favorites, type FavoriteContainer } from '$lib/stores/favorites';
	import type { ContainerInfo, ImageUpdateCheck } from '$lib/api/types';

	let containers = $state<ContainerInfo[]>([]);
	let loading = $state(true);
	let search = $state('');
	let page = $state(1);
	let perPage = $state(10);
	let selected = $state<Set<string>>(new Set());
	let bulkRunning = $state(false);
	let confirm = $state<{ message: string; action: () => void } | null>(null);
	let updateStatus = $state<Map<string, 'checking' | 'up-to-date' | 'outdated'>>(new Map());
	interface RecreateStep { text: string; status: 'pending' | 'running' | 'done' | 'error'; detail?: string; }
	let recreateModal = $state<{ name: string; image: string; steps: RecreateStep[]; output: string; done: boolean } | null>(null);

	let migrateModal = $state<{ id: string; name: string; image: string } | null>(null);
	let migrateTarget = $state('');
	let migrateStopSource = $state(true);
	let migrating = $state(false);
	let migrateDropdown = $state(false);

	let rollbackModal = $state<{ id: string; name: string } | null>(null);
	let snapshots = $state<{id: number; image: string; created_at: string}[]>([]);
	let rollingBack = $state(false);
	let diffData = $state<{changes: {field: string; old: string; new: string}[]; snapshot1_image: string; snapshot2_image: string} | null>(null);

	const migrateTargets = $derived(
		$environments.filter(e => e.id !== $selectedEnv).map(e => ({ value: e.id, label: e.name }))
	);

	let sortKey = $state<string>('name');
	let sortAsc = $state(true);

	function toggleSort(key: string) {
		if (sortKey === key) { sortAsc = !sortAsc; }
		else { sortKey = key; sortAsc = false; }
	}

	function sortIndicator(key: string): string {
		if (sortKey !== key) return '';
		return sortAsc ? ' ▲' : ' ▼';
	}

	const filtered = $derived(
		containers
			.filter(c =>
				c.name.toLowerCase().includes(search.toLowerCase()) ||
				c.image.toLowerCase().includes(search.toLowerCase())
			)
			.sort((a, b) => {
				const av = (a as any)[sortKey];
				const bv = (b as any)[sortKey];
				if (typeof av === 'number' && typeof bv === 'number') return sortAsc ? av - bv : bv - av;
				return sortAsc ? String(av ?? '').localeCompare(String(bv ?? '')) : String(bv ?? '').localeCompare(String(av ?? ''));
			})
	);
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));
	const allSelected = $derived(paged.length > 0 && paged.every(c => selected.has(c.id)));
	const someSelected = $derived(selected.size > 0);

	onMount(() => load());
	$effect(() => { $selectedEnv; load(); });

	let skipNextUpdateCheck: string | false = false;

	async function load() {
		if (!$selectedEnv) return;
		loading = true; selected = new Set();
		const r = await api.get<ContainerInfo[]>(`/env/${$selectedEnv}/containers`);
		if (r.success) {
			containers = r.data || [];
			if (skipNextUpdateCheck) {
				const img = skipNextUpdateCheck;
				skipNextUpdateCheck = false;
				const m = new Map(updateStatus);
				for (const c of containers) {
					if (c.image === img) m.set(c.id, 'up-to-date');
				}
				updateStatus = m;
			} else {
				checkAllInBackground();
			}
		}
		loading = false;
	}

	async function checkAllInBackground() {
		for (const c of containers) {
			if (c.state === 'running') {
				checkUpdate(c.id, true);
			}
		}
	}

	async function checkUpdate(id: string, silent = false) {
		const m = new Map(updateStatus); m.set(id, 'checking'); updateStatus = m;
		const r = await api.post<ImageUpdateCheck>(`/env/${$selectedEnv}/containers/${id}/check-update`, {});
		const m2 = new Map(updateStatus);
		if (r.success && r.data) {
			m2.set(id, r.data.outdated ? 'outdated' : 'up-to-date');
			if (!silent && r.data.outdated) toasts.success($t('containers.updateAvailable', { image: r.data.image }));
		} else {
			m2.delete(id);
			if (!silent) toasts.error(r.error || $t('containers.checkFailed'));
		}
		updateStatus = m2;
	}

	function toggleSelect(id: string) { const s = new Set(selected); if (s.has(id)) s.delete(id); else s.add(id); selected = s; }
	function toggleAll() { selected = allSelected ? new Set() : new Set(paged.map(c => c.id)); }
	$effect(() => { search; page = 1; });
	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }

	async function action(id: string, act: string) {
		if (act === 'remove') { confirm = { message: $t('containers.confirmDelete'), action: async () => { confirm = null; await doAction(id, act); } }; return; }
		await doAction(id, act);
	}
	async function doAction(id: string, act: string) {
		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${id}/action`, { action: act });
		if (r.success) { toasts.success($t('containers.successAction', { action: act })); setTimeout(load, 500); } else toasts.error(r.error || $t('common.error'));
	}

	function setStep(idx: number, status: RecreateStep['status'], detail?: string) {
		if (!recreateModal) return;
		recreateModal.steps[idx].status = status;
		if (detail) recreateModal.steps[idx].detail = detail;
		recreateModal = { ...recreateModal };
	}

	async function recreate(id: string) {
		const c = containers.find(x => x.id === id);
		const name = c?.name || id.substring(0, 12);
		const image = c?.image || '?';

		recreateModal = {
			name, image, output: '', done: false,
			steps: [
				{ text: $t('containers.pullImage'), status: 'running' },
				{ text: $t('containers.stopRemove'), status: 'pending' },
				{ text: $t('containers.createStart'), status: 'pending' },
			]
		};

		// Simulate step progress while waiting for the API
		const stepTimer = setInterval(() => {
			if (!recreateModal || recreateModal.done) { clearInterval(stepTimer); return; }
			const running = recreateModal.steps.findIndex(s => s.status === 'running');
			if (running >= 0 && running < recreateModal.steps.length - 1) {
				// Move to next step after a delay
			}
		}, 2000);

		// Step 1→2 after 3s, 2→3 after 6s (simulated, actual work happens in one API call)
		setTimeout(() => { if (recreateModal && !recreateModal.done) { setStep(0, 'done'); setStep(1, 'running'); } }, 3000);
		setTimeout(() => { if (recreateModal && !recreateModal.done) { setStep(1, 'done'); setStep(2, 'running'); } }, 6000);

		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${id}/recreate`, {});
		clearInterval(stepTimer);

		if (r.success) {
			recreateModal.steps.forEach((s, i) => { s.status = 'done'; });
			recreateModal.output = r.data || '';
			recreateModal.done = true;
			recreateModal = { ...recreateModal };
			// Mark this container (and others with same image) as up-to-date
			const img = containers.find(x => x.id === id)?.image;
			const m = new Map(updateStatus);
			if (img) {
				for (const c of containers) {
					if (c.image === img) m.set(c.id, 'up-to-date');
				}
			}
			m.set(id, 'up-to-date');
			updateStatus = m;
			skipNextUpdateCheck = img || 'unknown';
			setTimeout(load, 1500);
		} else {
			const failIdx = recreateModal.steps.findIndex(s => s.status === 'running');
			if (failIdx >= 0) recreateModal.steps[failIdx].status = 'error';
			recreateModal.output = r.error || $t('common.error');
			recreateModal.done = true;
			recreateModal = { ...recreateModal };
		}
	}

	async function bulkAction(act: string) {
		if (selected.size === 0) return;
		if (act === 'remove') { confirm = { message: $t('containers.confirmBulkDelete', { count: selected.size }), action: () => { confirm = null; doBulk(act); } }; return; }
		if (act === 'recreate') { confirm = { message: $t('containers.confirmBulkRecreate', { count: selected.size }), action: () => { confirm = null; doBulk(act); } }; return; }
		await doBulk(act);
	}
	async function doBulk(act: string) {
		if (act === 'recreate') {
			const ids = [...selected];
			const steps: RecreateStep[] = ids.map(id => {
				const c = containers.find(x => x.id === id);
				return { text: c?.name || id.substring(0, 12), status: 'pending' as const };
			});
			recreateModal = { name: `${ids.length} ${$t('containers.title')}`, image: 'Bulk Recreate', steps, output: '', done: false };

			let ok = 0, fail = 0;
			for (let i = 0; i < ids.length; i++) {
				setStep(i, 'running');
				const r = await api.post<string>(`/env/${$selectedEnv}/containers/${ids[i]}/recreate`, {});
				if (r.success) { ok++; setStep(i, 'done', r.data); }
				else { fail++; setStep(i, 'error', r.error); }
			}
			recreateModal.output = $t('containers.successCount', { ok, fail });
			recreateModal.done = true;
			recreateModal = { ...recreateModal };
			selected = new Set();
			setTimeout(load, 1500);
			return;
		}
		bulkRunning = true;
		let ok = 0, fail = 0;
		for (const id of selected) {
			const r = await api.post<string>(`/env/${$selectedEnv}/containers/${id}/action`, { action: act });
			if (r.success) ok++; else fail++;
		}
		bulkRunning = false;
		if (ok > 0) toasts.success($t('containers.bulkSuccess', { count: ok, action: act }));
		if (fail > 0) toasts.error($t('containers.bulkFail', { count: fail, action: act }));
		selected = new Set(); setTimeout(load, 800);
	}

	async function migrateContainer() {
		if (!migrateModal || !migrateTarget) return;
		migrating = true;
		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${migrateModal.id}/migrate`, {
			target_env_id: migrateTarget,
			stop_source: migrateStopSource,
		});
		migrating = false;
		if (r.success) {
			toasts.success(r.data || $t('containers.migrated'));
			migrateModal = null;
			migrateTarget = '';
			load();
		} else {
			toasts.error(r.error || $t('common.error'));
		}
	}

	async function loadSnapshots(name: string) {
		const r = await api.get<{id: number; image: string; created_at: string}[]>(`/snapshots/${encodeURIComponent(name)}`);
		if (r.success && r.data) snapshots = r.data;
		else snapshots = [];
	}

	async function deleteSnapshot(snapshotId: number) {
		const r = await api.del<string>(`/snapshots/delete/${snapshotId}`);
		if (r.success) { snapshots = snapshots.filter(s => s.id !== snapshotId); toasts.success($t('containers.snapshotDeleted')); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function showDiff(snapId: number) {
		if (snapshots.length < 2) return;
		const currentIdx = snapshots.findIndex(s => s.id === snapId);
		const compareWith = currentIdx < snapshots.length - 1 ? snapshots[currentIdx + 1].id : snapshots[0].id;
		if (snapId === compareWith) return;
		const r = await api.get<any>(`/snapshots/diff/${compareWith}/${snapId}`);
		if (r.success && r.data) diffData = r.data;
	}

	async function doRollback(snapshotId: number) {
		if (!rollbackModal) return;
		const cId = rollbackModal.id;
		const cName = rollbackModal.name;
		const snap = snapshots.find(s => s.id === snapshotId);

		// Close rollback list, show progress modal
		rollbackModal = null; snapshots = [];
		recreateModal = {
			name: cName, image: snap?.image || '?', output: '', done: false,
			steps: [
				{ text: $t('containers.rollbackPull'), status: 'running' },
				{ text: $t('containers.stopRemove'), status: 'pending' },
				{ text: $t('containers.rollbackRestore'), status: 'pending' },
			]
		};

		setTimeout(() => { if (recreateModal && !recreateModal.done) { recreateModal.steps[0].status = 'done'; recreateModal.steps[1].status = 'running'; recreateModal = { ...recreateModal }; } }, 3000);
		setTimeout(() => { if (recreateModal && !recreateModal.done) { recreateModal.steps[1].status = 'done'; recreateModal.steps[2].status = 'running'; recreateModal = { ...recreateModal }; } }, 6000);

		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${cId}/rollback`, { snapshot_id: snapshotId });
		if (r.success) {
			recreateModal!.steps.forEach(s => { s.status = 'done'; });
			recreateModal!.output = r.data || '';
			recreateModal!.done = true;
			recreateModal = { ...recreateModal! };
			setTimeout(load, 1500);
		} else {
			const failIdx = recreateModal!.steps.findIndex(s => s.status === 'running');
			if (failIdx >= 0) recreateModal!.steps[failIdx].status = 'error';
			recreateModal!.output = r.error || $t('common.error');
			recreateModal!.done = true;
			recreateModal = { ...recreateModal! };
		}
	}
</script>

<svelte:head><title>DockPit — Container</title></svelte:head>

<!-- Pinned Containers -->
{#if !loading && $favorites.length > 0}
	<div class="mb-4">
		<h3 class="text-sm font-semibold text-primary mb-2">{$t('favorites.pinned')}</h3>
		<div class="flex flex-wrap gap-3">
			{#each $favorites.filter(f => f.envId === $selectedEnv) as fav}
				{@const fc = containers.find(c => c.id === fav.id)}
				<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3 min-w-[220px]">
					<div class="flex-1 min-w-0">
						<div class="text-sm font-medium text-primary truncate">{fav.name}</div>
						<div class="text-[10px] text-muted truncate max-w-[140px]">{fav.image}</div>
					</div>
					{#if fc}
						<Badge status={fc.state} health={extractHealth(fc.status)} />
					{:else}
						<span class="text-[10px] text-muted">—</span>
					{/if}
					<div class="flex gap-1">
						{#if $canEditContainers && fc}
							{#if fc.state !== 'running'}
								<button class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" onclick={() => action(fav.id, 'start')} title={$t('containers.start')}>
									<svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></button>
							{:else}
								<button class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" onclick={() => action(fav.id, 'stop')} title={$t('containers.stop')}>
									<svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></button>
							{/if}
						{/if}
						{#if $canManageDocker}
							<a href="/containers/{fav.id}/logs" class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition no-underline" title={$t('containers.logs')}>
								<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg></a>
							<a href="/containers/{fav.id}/terminal" class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--green)] hover:border-[var(--green)] transition no-underline" title={$t('containers.terminal')}>
								<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg></a>
						{/if}
						<button class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--yellow)] hover:text-[var(--text-muted)] transition" onclick={() => favorites.remove(fav.id)} title={$t('favorites.unpin')}>
							<svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
						</button>
					</div>
				</div>
			{/each}
		</div>
	</div>
{/if}

<!-- Stats -->
{#if !loading}
	{@const running = containers.filter(c => c.state === 'running').length}
	{@const healthy = containers.filter(c => c.status.includes('(healthy)')).length}
	{@const unhealthy = containers.filter(c => c.status.includes('(unhealthy)')).length}
	{@const stopped = containers.filter(c => c.state === 'exited' || c.state === 'dead').length}
	{@const other = containers.length - running - stopped}
	<div class="grid grid-cols-2 md:grid-cols-5 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{containers.length}</div>
			<div class="text-[11px] text-secondary">{$t('containers.total')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{running}</div>
			<div class="text-[11px] text-secondary">{$t('containers.active')}{#if healthy > 0}<span class="text-[10px] text-green ml-1">({healthy} healthy)</span>{/if}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-red">{stopped}</div>
			<div class="text-[11px] text-secondary">{$t('containers.stopped')}</div>
		</div>
		{#if unhealthy > 0}
			<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3 border-[var(--red)]">
				<div class="text-xl font-bold text-red">{unhealthy}</div>
				<div class="text-[11px] text-red">{$t('containers.unhealthy')}</div>
			</div>
		{:else}
			<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
				<div class="text-xl font-bold text-green">0</div>
				<div class="text-[11px] text-secondary">{$t('containers.unhealthy')}</div>
			</div>
		{/if}
		{#if other > 0}
			<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
				<div class="text-xl font-bold text-yellow">{other}</div>
				<div class="text-[11px] text-secondary">{$t('containers.other')}</div>
			</div>
		{:else}
			<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
				<div class="text-xl font-bold text-accent">0</div>
				<div class="text-[11px] text-secondary">{$t('containers.other')}</div>
			</div>
		{/if}
	</div>
{/if}

<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<h3 class="text-sm font-semibold text-primary">{$t('containers.title')} ({filtered.length})</h3>
		<div class="flex items-center gap-2">
			<input bind:value={search} placeholder={$t('common.search')} class="bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs w-40 focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200" />
			{#if $canManageDocker}<Button variant="warning" size="sm" onclick={() => { for (const c of filtered) if (c.state === 'running') checkUpdate(c.id); }} title={$t('containers.checkUpdates')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
				{$t('containers.checkUpdates')}
			</Button>{/if}
			<Button variant="success" size="sm" onclick={load} title={$t('common.refresh')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</Button>
		</div>
	</div>

	{#if someSelected && $canEditContainers}
		<div class="px-4 py-2 border-b border-theme bg-1 flex items-center gap-2 flex-wrap">
			<span class="text-xs text-secondary">{$t('common.selected', { count: selected.size })}</span>
			<div class="flex gap-1.5 ml-2">
				<Button variant="success" size="sm" onclick={() => bulkAction('start')} disabled={bulkRunning}>{$t('containers.start')}</Button>
				<Button variant="warning" size="sm" onclick={() => bulkAction('stop')} disabled={bulkRunning}>{$t('containers.stop')}</Button>
				<Button variant="primary" size="sm" onclick={() => bulkAction('restart')} disabled={bulkRunning}>{$t('containers.restart')}</Button>
				<Button variant="purple" size="sm" onclick={() => bulkAction('recreate')} disabled={bulkRunning}>{$t('containers.recreate')}</Button>
				<Button variant="danger" size="sm" onclick={() => bulkAction('remove')} disabled={bulkRunning}>{$t('containers.remove')}</Button>
			</div>
			{#if bulkRunning}<div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin ml-2"></div>{/if}
		</div>
	{/if}

	{#if loading}
		<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="w-10 px-4 py-2"></th>
					<th class="w-10 px-4 py-2"><CustomCheckbox checked={allSelected} onchange={toggleAll} size="sm" /></th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('name')}>{$t('common.name')}{sortIndicator('name')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden lg:table-cell cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('stack_name')}>Stack{sortIndicator('stack_name')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('image')}>Image{sortIndicator('image')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('state')}>{$t('common.status')}{sortIndicator('state')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden xl:table-cell cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('ip_address')}>IP{sortIndicator('ip_address')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden lg:table-cell">Ports</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden xl:table-cell cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('created')}>{$t('images.created')}{sortIndicator('created')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as c}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition {selected.has(c.id) ? 'bg-accent-light' : ''}">
							<td class="w-10 px-4 py-2.5">
								<button class="flex items-center justify-center {$favorites.some(f => f.id === c.id) ? 'text-[var(--yellow)]' : 'text-[var(--text-muted)] hover:text-[var(--yellow)]'} transition" onclick={() => favorites.toggle({id: c.id, name: c.name, envId: $selectedEnv, image: c.image})} title={$favorites.some(f => f.id === c.id) ? $t('favorites.unpin') : $t('favorites.pin')}>
									<svg class="w-4 h-4" viewBox="0 0 24 24" fill="{$favorites.some(f => f.id === c.id) ? 'currentColor' : 'none'}" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
								</button>
							</td>
							<td class="w-10 px-4 py-2.5"><CustomCheckbox checked={selected.has(c.id)} onchange={() => toggleSelect(c.id)} size="sm" /></td>
							<td class="px-4 py-2.5">
								<div class="text-sm font-medium text-primary">{c.name}</div>
								<div class="text-[10px] font-mono text-muted">{truncateId(c.id)}</div>
							</td>
							<td class="px-4 py-2.5 hidden lg:table-cell">
								{#if c.stack_name}
									<a href="/stacks/{c.stack_name}" class="text-xs text-accent hover:text-accent-hover transition">{c.stack_name}</a>
								{:else}<span class="text-xs text-muted">—</span>{/if}
							</td>
							<td class="px-4 py-2.5 hidden md:table-cell">
								<div class="flex items-center gap-1.5">
									<!-- Update indicator -->
									{#if updateStatus.get(c.id) === 'checking'}
										<div class="w-3 h-3 border border-theme border-t-[var(--accent)] rounded-full animate-spin shrink-0"></div>
									{:else if updateStatus.get(c.id) === 'up-to-date'}
										<span class="w-4 h-4 rounded-full bg-[var(--green)] flex items-center justify-center shrink-0" title={$t('containers.imageUpToDate')}>
											<svg class="w-2.5 h-2.5 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
										</span>
									{:else if updateStatus.get(c.id) === 'outdated'}
										<span class="w-4 h-4 rounded-full bg-[var(--red)] flex items-center justify-center shrink-0 cursor-pointer" title={$t('containers.updateClick')} onclick={() => recreate(c.id)}>
											<svg class="w-2.5 h-2.5 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
										</span>
									{:else}
										<span class="w-4 h-4 rounded-full bg-3 border border-theme shrink-0" title={$t('containers.notChecked')}></span>
									{/if}
									<span class="text-xs text-secondary max-w-[160px] truncate">{c.image}</span>
								</div>
							</td>
							<td class="px-4 py-2.5"><Badge status={c.state} health={extractHealth(c.status)} /></td>
							<td class="px-4 py-2.5 text-[11px] font-mono text-secondary hidden xl:table-cell">{c.ip_address || '—'}</td>
							<td class="px-4 py-2.5 text-[11px] font-mono text-secondary hidden lg:table-cell">{formatPorts(c.ports)}</td>
							<td class="px-4 py-2.5 text-[11px] text-secondary hidden xl:table-cell">{formatDate(c.created)}</td>
							<td class="px-4 py-2.5">
								<div class="flex gap-1">
									<!-- Start/Stop (editor+) -->
									{#if $canEditContainers}
										{#if c.state !== 'running'}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--green)] hover:border-[var(--green)]/40 hover:bg-[var(--green)]/8 transition" onclick={() => action(c.id, 'start')} title={$t('containers.start')}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></button>
										{:else}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => action(c.id, 'stop')} title={$t('containers.stop')}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></button>
										{/if}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition" onclick={() => action(c.id, 'restart')} title={$t('containers.restart')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/></svg></button>
									{/if}
									<!-- Recreate/Logs/Terminal/Delete (admin+) -->
									{#if $canManageDocker}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition" onclick={() => recreate(c.id)} title={$t('containers.recreate')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg></button>
										<a href="/containers/{c.id}/logs" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition no-underline" title={$t('containers.logs')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg></a>
										<a href="/containers/{c.id}/terminal" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--green)] hover:border-[var(--green)]/40 hover:bg-[var(--green)]/8 transition no-underline" title={$t('containers.terminal')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg></a>
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--yellow)] hover:border-[var(--yellow)]/40 hover:bg-[var(--yellow)]/8 transition" onclick={async () => { rollbackModal = { id: c.id, name: c.name }; await loadSnapshots(c.name); }} title={$t('containers.rollback')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12a9 9 0 109-9"/><polyline points="3 3 3 9 9 9"/><path d="M3 9l3-3"/></svg></button>
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition" onclick={() => migrateModal = { id: c.id, name: c.name, image: c.image }} title={$t('containers.migrate')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M12 5l7 7-7 7"/></svg></button>
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => action(c.id, 'remove')} title={$t('containers.remove')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg></button>
									{/if}
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="11" class="text-center py-10 text-sm text-muted">{$t('containers.noContainers')}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
		<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
	{/if}
</div>

{#if recreateModal}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[1000] p-4"
		onclick={(e) => { if (e.target === e.currentTarget && recreateModal?.done) recreateModal = null; }}>
		<div class="bg-card border border-theme rounded-xl w-full max-w-xl shadow-2xl flex flex-col max-h-[85vh]">
			<!-- Header -->
			<div class="flex items-center justify-between px-6 py-4 border-b border-theme shrink-0">
				<div>
					<h3 class="text-base font-semibold text-primary">{$t('containers.recreateTitle')}</h3>
					<p class="text-xs text-secondary mt-0.5">{recreateModal.name} · {recreateModal.image}</p>
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
						<!-- Status icon -->
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
						<!-- Text -->
						<div class="flex-1 min-w-0">
							<p class="text-sm {step.status === 'running' ? 'text-primary font-medium' : step.status === 'done' ? 'text-secondary' : step.status === 'error' ? 'text-red' : 'text-muted'}">
								{step.text}
							</p>
							{#if step.detail}
								<p class="text-[11px] text-muted mt-1 font-mono break-all">{step.detail}</p>
							{/if}
						</div>
					</div>
					{#if i < recreateModal.steps.length - 1}
						<div class="ml-3 w-px h-2 bg-3"></div>
					{/if}
				{/each}
			</div>

			<!-- Output (collapsible) -->
			{#if recreateModal.output && recreateModal.done}
				<div class="border-t border-theme">
					<details class="group">
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

{#if migrateModal}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1000] p-0 sm:p-4" onclick={(e) => { if (e.target === e.currentTarget) { migrateModal = null; migrateDropdown = false; } }}>
		<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-lg shadow-[var(--shadow-lg)]" style="overflow:visible; background:var(--glass-bg); backdrop-filter:blur(20px) saturate(150%); -webkit-backdrop-filter:blur(20px) saturate(150%)">
			<div class="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
				<h3 class="text-[15px] font-semibold text-[var(--text)]">{$t('containers.migrateTitle')}</h3>
				<button class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition-all" onclick={() => { migrateModal = null; migrateDropdown = false; }}>
					<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>
			<div class="p-5 space-y-4">
				<div>
					<p class="text-sm text-secondary mb-1">{$t('containers.migrateContainer')}</p>
					<p class="text-sm font-medium text-primary">{migrateModal.name}</p>
					<p class="text-xs text-muted mt-0.5">{migrateModal.image}</p>
				</div>
				<div>
					<label class="block text-xs font-medium text-secondary mb-1">{$t('containers.migrateTarget')}</label>
					<div class="relative">
						<button
							class="w-full flex items-center gap-2.5 h-9 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-3)] text-[var(--text)] hover:border-[var(--border-light)] hover:shadow-[var(--shadow-sm)] transition-all cursor-pointer"
							onclick={(e) => { e.stopPropagation(); migrateDropdown = !migrateDropdown; }}
						>
							{#if migrateTarget}
								{@const env = $environments.find(e => e.id === migrateTarget)}
								<span class="w-2 h-2 rounded-full shrink-0 {env?.status === 'online' || env?.is_local ? 'bg-[var(--green)] shadow-[var(--shadow-glow-green)]' : 'bg-[var(--red)]'}"></span>
								<span class="truncate font-medium">{env?.name}</span>
							{:else}
								<span class="text-muted">{$t('containers.selectTarget')}</span>
							{/if}
							<svg class="w-3.5 h-3.5 text-muted shrink-0 ml-auto transition-transform duration-200 {migrateDropdown ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
						</button>
						{#if migrateDropdown}
							<div class="absolute left-0 right-0 mt-1.5 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] z-[1100] py-1.5">
								<div class="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)]">{$t('containers.migrateTarget')}</div>
								{#each migrateTargets as t}
									{@const env = $environments.find(e => e.id === t.value)}
									<button
										class="w-full flex items-center gap-3 px-3 py-2.5 text-xs text-left transition-all duration-150
										{t.value === migrateTarget ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text)]'}"
										onclick={() => { migrateTarget = t.value; migrateDropdown = false; }}
									>
										<span class="w-2 h-2 rounded-full shrink-0 {env?.status === 'online' || env?.is_local ? 'bg-[var(--green)]' : 'bg-[var(--red)]'}"></span>
										<span class="truncate font-medium">{t.label}</span>
										{#if env?.is_local}
											<span class="text-[9px] text-muted ml-auto px-1.5 py-0.5 rounded-full bg-[var(--bg-3)]">Local</span>
										{/if}
										{#if t.value === migrateTarget}
											<svg class="w-3.5 h-3.5 shrink-0 ml-auto text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
										{/if}
									</button>
								{/each}
							</div>
						{/if}
					</div>
				</div>
				<CustomCheckbox checked={migrateStopSource} onchange={(v) => migrateStopSource = v} label={$t('containers.migrateStopSource')} />
			</div>
			<div class="px-5 py-3 border-t border-[var(--border)] flex justify-end gap-2">
				<Button variant="danger" size="sm" onclick={() => { migrateModal = null; migrateDropdown = false; }}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" onclick={migrateContainer} loading={migrating} disabled={!migrateTarget}>{$t('containers.migrateStart')}</Button>
			</div>
		</div>
	</div>
{/if}

{#if rollbackModal}
	<Modal title={$t('containers.rollbackTitle')} onclose={() => { rollbackModal = null; snapshots = []; diffData = null; }}>
		<div class="space-y-3">
			<p class="text-sm text-secondary">{$t('containers.rollbackDesc')}</p>
			<p class="text-sm font-medium text-primary">{rollbackModal.name}</p>
			{#if snapshots.length === 0}
				<div class="text-center py-6 text-sm text-muted">{$t('containers.noSnapshots')}</div>
			{:else}
				<div class="space-y-2 max-h-[300px] overflow-y-auto">
					{#each snapshots as snap}
						<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3">
							<div class="flex items-center justify-between gap-3">
								<div class="min-w-0 flex-1">
									<div class="text-xs font-mono text-primary truncate">{snap.image}</div>
									<div class="text-[10px] text-muted mt-0.5">{formatDateTime(snap.created_at)}</div>
								</div>
								<div class="flex items-center gap-1.5 shrink-0">
									{#if snapshots.length >= 2}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-muted hover:text-[var(--accent)] hover:border-[var(--accent)] transition" onclick={() => showDiff(snap.id)} title={$t('containers.compare')}>
											<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 3h5v5M4 20L21 3M21 16v5h-5M15 15l6 6M4 4l5 5"/></svg>
										</button>
									{/if}
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition" onclick={() => doRollback(snap.id)} title={$t('containers.rollbackTo')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12a9 9 0 109-9"/><polyline points="3 3 3 9 9 9"/><path d="M3 9l3-3"/></svg>
									</button>
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => deleteSnapshot(snap.id)} title={$t('common.delete')}>
										<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
									</button>
								</div>
							</div>
						</div>
					{/each}
				</div>
			{/if}
			{#if diffData}
				<div class="mt-3 border-t border-theme pt-3">
					<div class="flex items-center justify-between mb-2">
						<h4 class="text-xs font-semibold text-primary">{$t('containers.changes')} ({diffData.changes.length})</h4>
						<button class="text-[10px] text-muted hover:text-primary" onclick={() => diffData = null}>{$t('common.close')}</button>
					</div>
					{#if diffData.changes.length === 0}
						<p class="text-xs text-muted">{$t('containers.noChanges')}</p>
					{:else}
						<div class="space-y-1.5 max-h-[200px] overflow-y-auto">
							{#each diffData.changes as change}
								<div class="text-[11px] bg-[var(--bg-0)] rounded p-2">
									<span class="font-medium text-primary">{change.field}</span>
									{#if change.old}<span class="text-[var(--red)] line-through ml-2">{change.old}</span>{/if}
									{#if change.new}<span class="text-[var(--green)] ml-2">{change.new}</span>{/if}
								</div>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</Modal>
{/if}

{#if confirm}
	<ConfirmDialog message={confirm.message} onconfirm={confirm.action} oncancel={() => confirm = null} />
{/if}
