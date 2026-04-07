<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { canSeePage } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import { formatDateTimeSmart } from '$lib/utils/format';
	import { initResizableColumns } from '$lib/utils/resizable-columns';
	import type { ContainerEvent, EventsResponse } from '$lib/api/types';

	$effect(() => {
		if (!$canSeePage('page.events')) goto('/profile');
	});

	let tableEl: HTMLTableElement | undefined = $state();
	let events = $state<ContainerEvent[]>([]);
	let loading = $state(true);
	let total = $state(0);
	let page = $state(1);
	let perPage = $state(25);
	let filter = $state<string>('all');
	let refreshInterval: ReturnType<typeof setInterval> | undefined;
	let refreshing = $state(false);

	const actionOptions = $derived([
		{ value: 'all', label: $t('events.all') },
		{ value: 'start', label: $t('events.start') },
		{ value: 'stop', label: $t('events.stop') },
		{ value: 'die', label: $t('events.die') },
		{ value: 'kill', label: $t('events.kill') },
		{ value: 'restart', label: $t('events.restart') },
		{ value: 'oom', label: $t('events.oom') },
		{ value: 'create', label: $t('events.create') },
		{ value: 'destroy', label: $t('events.destroy') },
		{ value: 'health_status', label: $t('events.health_status') },
	]);

	const filtered = $derived(
		filter === 'all' ? events : events.filter(e => e.event_action === filter)
	);
	const paged = $derived(
		perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage)
	);

	onMount(() => {
		loadEvents();
		refreshInterval = setInterval(() => loadEvents(), 10000);
	});

	onDestroy(() => {
		if (refreshInterval) clearInterval(refreshInterval);
	});

	$effect(() => { $selectedEnv; loadEvents(); });
	$effect(() => { filter; page = 1; });
	$effect(() => { if (tableEl && !loading && events.length > 0) initResizableColumns(tableEl); });

	async function loadEvents() {
		if (!$selectedEnv) return;
		const r = await api.get<EventsResponse>(`/env/${$selectedEnv}/events?limit=500&offset=0`);
		if (r.success && r.data) {
			events = r.data.events || [];
			total = r.data.total;
		}
		loading = false;
	}

	async function refresh() {
		if (!$selectedEnv || refreshing) return;
		refreshing = true;
		await api.post(`/env/${$selectedEnv}/events/refresh`, {});
		await loadEvents();
		refreshing = false;
	}

	function handlePageChange(p: number, pp: number) {
		page = p;
		perPage = pp;
	}

	function badgeClass(action: string): string {
		switch (action) {
			case 'start': return 'bg-[var(--green-bg)] text-[var(--green)]';
			case 'stop': return 'bg-[var(--yellow-bg)] text-[var(--yellow)]';
			case 'die': case 'kill': case 'oom': return 'bg-[var(--red-bg)] text-[var(--red)]';
			case 'restart': return 'bg-[var(--accent-bg)] text-[var(--accent)]';
			case 'create': case 'destroy': return 'bg-[var(--bg-hover)] text-[var(--text-muted)]';
			case 'health_status': return 'bg-[#f3e8ff] text-[#9333ea] dark:bg-[#9333ea20] dark:text-[#c084fc]';
			default: return 'bg-[var(--bg-hover)] text-[var(--text-secondary)]';
		}
	}

	function actionLabel(action: string): string {
		const key = `events.${action}` as any;
		const translated = $t(key);
		return translated !== key ? translated : action;
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
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('events.title')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{filtered.length} {$t('events.title').toLowerCase()}</p>
		</div>
		<div class="flex items-center gap-2">
			<div class="flex items-center gap-1.5 text-[11px] text-[var(--text-muted)]">
				<span class="relative flex h-2 w-2">
					<span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-[var(--green)] opacity-75"></span>
					<span class="relative inline-flex rounded-full h-2 w-2 bg-[var(--green)]"></span>
				</span>
				{$t('events.autoRefresh')}
			</div>
			<Button size="sm" variant="success" onclick={refresh} disabled={refreshing} title={$t('events.refresh')}>
				<svg class="w-3.5 h-3.5 {refreshing ? 'animate-spin' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6"/><path d="M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</Button>
		</div>
	</div>

	<!-- Filter bar -->
	<div class="flex items-center gap-3">
		<span class="text-xs text-[var(--text-muted)]">{$t('events.filter')}:</span>
		<CustomSelect
			options={actionOptions}
			value={filter}
			onchange={(v) => { filter = String(v); }}
			size="sm"
			class="w-[180px]"
		/>
	</div>

	<!-- Table -->
	<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] overflow-hidden">
		{#if loading}
			<div class="flex items-center justify-center py-16">
				<div class="w-6 h-6 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
			</div>
		{:else if filtered.length === 0}
			<div class="flex flex-col items-center justify-center py-16 text-center">
				<svg class="w-10 h-10 text-[var(--text-muted)] mb-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 8v4l3 3"/><circle cx="12" cy="12" r="10"/></svg>
				<p class="text-sm font-medium text-[var(--text-secondary)]">{$t('events.noEvents')}</p>
				<p class="text-xs text-[var(--text-muted)] mt-1 max-w-[300px]">{$t('events.noEventsDesc')}</p>
			</div>
		{:else}
			<div class="overflow-x-auto">
				<table bind:this={tableEl} class="w-full text-sm">
					<thead>
						<tr class="border-b border-[var(--border)]">
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('events.time')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('events.container')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('events.event')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('events.details')}</th>
						</tr>
					</thead>
					<tbody>
						{#each paged as event (event.id ?? event.timestamp + event.container_id)}
							<tr class="border-b border-[var(--border)] last:border-0 hover:bg-[var(--bg-hover)] transition-colors duration-150">
								<td class="px-4 py-3 text-xs text-[var(--text-muted)] font-mono whitespace-nowrap">{formatDateTimeSmart(event.timestamp)}</td>
								<td class="px-4 py-3 text-xs text-[var(--text)] font-medium" title={event.container_name || ''}>{truncateName(event.container_name || '')}</td>
								<td class="px-4 py-3">
									<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium {badgeClass(event.event_action)}">
										{actionLabel(event.event_action)}
									</span>
								</td>
								<td class="px-4 py-3 text-xs text-[var(--text-secondary)] max-w-[300px] truncate">{event.details || '-'}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>

			<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
		{/if}
	</div>
</div>
