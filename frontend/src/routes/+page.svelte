<script lang="ts">
	import { onMount, onDestroy, tick } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import { widgets, syncFromBackend, type WidgetConfig, getTabs, addTab, renameTab, removeTab, exportDashboard, importDashboard, type DashboardTab } from '$lib/stores/widgets';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import ServerCardWidget from '$lib/components/widgets/ServerCardWidget.svelte';
	import QuickStatsWidget from '$lib/components/widgets/QuickStatsWidget.svelte';
	import ResourcesWidget from '$lib/components/widgets/ResourcesWidget.svelte';
	import ContainerHealthWidget from '$lib/components/widgets/ContainerHealthWidget.svelte';
	import StackStatusWidget from '$lib/components/widgets/StackStatusWidget.svelte';
	import DiskUsageWidget from '$lib/components/widgets/DiskUsageWidget.svelte';
	import UptimeWidget from '$lib/components/widgets/UptimeWidget.svelte';
	import UnusedResourcesWidget from '$lib/components/widgets/UnusedResourcesWidget.svelte';
	import QuickActionsWidget from '$lib/components/widgets/QuickActionsWidget.svelte';
	import ResourceMonitorWidget from '$lib/components/widgets/ResourceMonitorWidget.svelte';
	import FavoritesWidget from '$lib/components/widgets/FavoritesWidget.svelte';
	import NoteWidget from '$lib/components/widgets/NoteWidget.svelte';
	import LinksWidget from '$lib/components/widgets/LinksWidget.svelte';
	import ClockWidget from '$lib/components/widgets/ClockWidget.svelte';
	import IframeWidget from '$lib/components/widgets/IframeWidget.svelte';
	import type { ServerOverview, SystemInfo } from '$lib/api/types';

	let servers = $state<ServerOverview[]>([]);
	let loading = $state(true);
	let showAddMenu = $state(false);
	let editMode = $state(false);
	let hasUnsavedChanges = $state(false);
	let addMenuStyle = $state('');
	let gridEl: HTMLElement;
	let grid: any = null;
	let tabs = $state<DashboardTab[]>(getTabs());
	let activeTabId = $state(getTabs()[0]?.id || 'default');
	let renamingTabId = $state<string | null>(null);
	let renameInput = $state('');
	let importFileEl: HTMLInputElement;

	const widgetColors = [
		{ name: 'default', value: '' },
		{ name: 'red', value: 'var(--red)' },
		{ name: 'green', value: 'var(--green)' },
		{ name: 'yellow', value: 'var(--yellow)' },
		{ name: 'purple', value: 'var(--purple)' },
		{ name: 'cyan', value: '#22d3ee' },
		{ name: 'orange', value: '#ff8a5c' },
		{ name: 'pink', value: '#f472b6' },
	];

	const customTypes = [
		{ type: 'note', label: $t('dashboard.note'), icon: '\u{1F4DD}' },
		{ type: 'links', label: $t('dashboard.links'), icon: '\u{1F517}' },
		{ type: 'clock', label: $t('dashboard.clock'), icon: '\u{1F550}' },
		{ type: 'iframe', label: $t('dashboard.iframe'), icon: '\u{1F5BC}' },
	];

	const filteredWidgets = $derived($widgets.filter(w => (w.tabId || 'default') === activeTabId));

	function positionAddMenu() {
		const anchor = document.getElementById('add-widget-anchor');
		if (!anchor) return;
		const rect = anchor.getBoundingClientRect();
		addMenuStyle = `top: ${rect.bottom + 8}px; right: ${window.innerWidth - rect.right}px;`;
	}

	const singletonTypes = [
		{ type: 'quick-stats', label: $t('home.widgetOverview'), icon: '📊' },
		{ type: 'resources', label: $t('home.widgetResources'), icon: '⚡' },
		{ type: 'container-health', label: $t('home.widgetHealth'), icon: '💚' },
		{ type: 'stack-status', label: $t('home.widgetStacks'), icon: '📦' },
		{ type: 'disk-usage', label: $t('home.widgetDisk'), icon: '💾' },
		{ type: 'uptime', label: $t('home.widgetUptime'), icon: '⏱' },
		{ type: 'unused-resources', label: $t('home.widgetCleanup'), icon: '🧹' },
		{ type: 'quick-actions', label: $t('home.widgetActions'), icon: '🚀' },
		{ type: 'resource-monitor', label: $t('monitoring.title'), icon: '📈' },
		{ type: 'favorites', label: $t('favorites.title'), icon: '⭐' },
	];

	function hasType(type: string): boolean { return $widgets.some(w => w.type === type); }
	const availableServers = $derived(servers.filter(s => !$widgets.some(w => w.type === 'server' && w.envId === s.id)));

	function getServer(envId?: string): ServerOverview | undefined { return servers.find(s => s.id === envId); }

	function widgetTitle(w: WidgetConfig): string {
		if (w.type === 'server') return getServer(w.envId)?.name || $t('home.servers');
		const found = singletonTypes.find(s => s.type === w.type) || customTypes.find(s => s.type === w.type);
		return found?.label || 'Widget';
	}

	onMount(async () => {
		const r = await api.get<ServerOverview[]>('/home/servers');
		if (r.success && r.data) servers = r.data;
		loading = false;
		// Load dashboard config from backend (if available), then init
		const synced = await syncFromBackend();
		if (synced) widgets.reload();
		widgets.init(servers.map(s => s.id));

		await tick();
		initGrid();

		for (let i = 0; i < servers.length; i++) {
			if (servers[i].info.status === 'loading') {
				api.get<SystemInfo>(`/env/${servers[i].id}/system`).then(sr => {
					if (sr.success && sr.data) {
						servers[i] = { ...servers[i], info: sr.data };
						servers = [...servers];
					} else {
						servers[i] = { ...servers[i], info: { ...servers[i].info, status: 'offline' } };
						servers = [...servers];
					}
				});
			}
		}
	});

	onDestroy(() => { grid?.destroy(false); });

	async function initGrid() {
		const { GridStack } = await import('gridstack');
		await import('gridstack/dist/gridstack.min.css');

		if (!gridEl) return;

		grid = GridStack.init({
			column: 6,
			cellHeight: 60,
			margin: 10,
			float: true,
			animate: true,
			draggable: { handle: '.widget-drag' },
			resizable: { handles: 'se,sw,ne,nw,e,w,s' },
			disableDrag: !editMode,
			disableResize: !editMode,
		}, gridEl);

		loadWidgetsToGrid();
		grid.on('change', () => {
			saveGridState();
			hasUnsavedChanges = true;
		});
	}

	function loadWidgetsToGrid() {
		if (!grid) return;
		const tabWidgets = $widgets.filter(w => (w.tabId || 'default') === activeTabId);
		grid.batchUpdate();
		grid.removeAll(false);
		for (const w of tabWidgets) {
			const el = document.getElementById(`widget-${w.id}`);
			if (el) {
				grid.makeWidget(el, { x: w.x, y: w.y, w: w.w, h: w.h, id: w.id });
			}
		}
		grid.batchUpdate(false);
	}

	function saveGridState() {
		if (!grid) return;
		const items = grid.getGridItems().map((el: HTMLElement) => ({
			id: el.getAttribute('gs-id') || '',
			x: parseInt(el.getAttribute('gs-x') || '0'),
			y: parseInt(el.getAttribute('gs-y') || '0'),
			w: parseInt(el.getAttribute('gs-w') || '2'),
			h: parseInt(el.getAttribute('gs-h') || '3'),
		}));
		widgets.updateLayout(items);
	}

	function toggleEditMode() {
		editMode = !editMode;
		tabs = getTabs(); // Sync tabs when toggling edit mode
		if (grid) {
			if (editMode) {
				grid.enableMove(true);
				grid.enableResize(true);
			} else {
				grid.enableMove(false);
				grid.enableResize(false);
			}
		}
		if (!editMode && hasUnsavedChanges) {
			saveGridState();
			hasUnsavedChanges = false;
			toasts.success($t('home.layoutSaved'));
		}
	}

	function saveDashboard() {
		saveGridState();
		hasUnsavedChanges = false;
		editMode = false;
		if (grid) {
			grid.enableMove(false);
			grid.enableResize(false);
		}
		// Sync tabs from storage to avoid ghost tabs
		tabs = getTabs();
		toasts.success($t('home.layoutSaved'));
	}

	async function addWidget(type: string, envId?: string) {
		showAddMenu = false;
		widgets.addWidget(type as any, envId, activeTabId);
		hasUnsavedChanges = true;
		await rebuildGrid();
	}

	async function removeWidget(id: string) {
		widgets.removeWidget(id);
		hasUnsavedChanges = true;
		await rebuildGrid();
	}

	async function resetLayout() {
		widgets.reset(servers.map(s => s.id));
		hasUnsavedChanges = false;
		await rebuildGrid();
		toasts.success($t('home.layoutReset'));
	}

	async function rebuildGrid() {
		if (grid) { grid.destroy(false); grid = null; }
		await tick();
		await tick();
		initGrid();
	}

	async function handleAddTab() {
		const tab = addTab($t('dashboard.newTab'));
		tabs = getTabs();
		activeTabId = tab.id;
		await rebuildGrid();
		// Auto-start rename so user can type the name immediately
		startRenameTab(tab.id, tab.name);
	}

	function startRenameTab(tabId: string, currentName: string) {
		renamingTabId = tabId;
		renameInput = currentName;
	}

	async function finishRenameTab() {
		if (renamingTabId && renameInput.trim()) {
			renameTab(renamingTabId, renameInput.trim());
			tabs = getTabs();
		}
		renamingTabId = null;
		renameInput = '';
	}

	async function handleRemoveTab(tabId: string) {
		removeTab(tabId);
		tabs = getTabs();
		if (activeTabId === tabId) {
			activeTabId = tabs[0]?.id || 'default';
		}
		await rebuildGrid();
	}

	async function switchTab(tabId: string) {
		if (tabId === activeTabId) return;
		activeTabId = tabId;
		await rebuildGrid();
	}

	function handleExport() {
		const json = exportDashboard();
		const blob = new Blob([json], { type: 'application/json' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `dockpit-dashboard-${Date.now()}.json`;
		a.click();
		URL.revokeObjectURL(url);
	}

	async function handleImport(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		const text = await file.text();
		if (importDashboard(text)) {
			tabs = getTabs();
			activeTabId = tabs[0]?.id || 'default';
			toasts.success($t('dashboard.imported'));
			await rebuildGrid();
		} else {
			toasts.error($t('dashboard.importError'));
		}
		input.value = '';
	}

	function setWidgetColor(widgetId: string, color: string) {
		widgets.updateWidget(widgetId, { color });
	}
</script>

<svelte:head><title>DockPit — {$t('home.dashboard')}</title></svelte:head>
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<svelte:window onclick={() => showAddMenu = false} />

<style>
	:global(.grid-stack) { min-height: 200px; z-index: 0; position: relative; }
	:global(.grid-stack-item) { z-index: 1 !important; }
	:global(.grid-stack-item-content) {
		background: var(--glass-bg);
		backdrop-filter: blur(16px) saturate(150%);
		-webkit-backdrop-filter: blur(16px) saturate(150%);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		overflow: hidden;
		display: flex;
		flex-direction: column;
		transition: border-color 0.2s, box-shadow 0.2s;
	}
	:global(.grid-stack-item-content:hover) {
		border-color: var(--border-light);
	}
	:global(.edit-mode .grid-stack-item-content) {
		border-color: var(--border-light);
		box-shadow: 0 0 0 1px var(--border-light), var(--shadow-sm);
	}
	:global(.edit-mode .grid-stack-item-content:hover) {
		box-shadow: var(--shadow-glow);
		border-color: var(--accent);
	}
	:global(.grid-stack > .grid-stack-item > .grid-stack-item-content) { inset: 5px; }
	:global(.gs-placeholder-content) {
		background: var(--accent-bg) !important;
		border: 2px dashed var(--accent) !important;
		border-radius: var(--radius-lg) !important;
	}
	:global(.ui-resizable-handle) { opacity: 0; transition: opacity 0.2s; }
	:global(.grid-stack-item:hover .ui-resizable-handle) { opacity: 0.6; }
	.edit-banner {
		background: var(--accent-bg);
		border: 1px solid rgba(108, 92, 231, 0.2);
	}
	.empty-state {
		background: var(--glass-bg);
		backdrop-filter: blur(16px);
		border: 1px dashed var(--border);
	}
	.add-menu-enter {
		animation: menu-open 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes menu-open {
		from { opacity: 0; transform: translateY(-8px) scale(0.96); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}
	.tab-bar {
		background: var(--glass-bg);
		backdrop-filter: blur(16px) saturate(150%);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
	}
	.tab-item {
		padding: 6px 14px;
		font-size: 11px;
		font-weight: 500;
		border-radius: var(--radius);
		transition: all 0.15s;
		cursor: pointer;
		white-space: nowrap;
	}
	.tab-item:hover { background: var(--bg-hover); }
	.tab-item.active {
		background: var(--accent-bg);
		color: var(--accent);
	}
	.color-dot {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		border: 2px solid transparent;
		cursor: pointer;
		transition: all 0.15s;
	}
	.color-dot:hover { transform: scale(1.2); }
	.color-dot.selected { border-color: var(--text); }
</style>

{#if loading}
	<div class="flex justify-center py-16">
		<div class="w-7 h-7 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
	</div>
{:else}
	<!-- Toolbar -->
	<div class="flex items-center justify-between mb-5 flex-wrap gap-3">
		<div>
			<h2 class="text-lg font-bold text-[var(--text)]">{$t('home.dashboard')}</h2>
			<p class="text-xs text-[var(--text-secondary)] mt-0.5">{servers.length} {$t('home.servers')} · {$widgets.length} {$t('home.widgets')}</p>
		</div>
		<div class="flex items-center gap-2">
			{#if editMode}
				<!-- Edit mode toolbar -->
				<div class="relative" id="add-widget-anchor">
					<Button variant="secondary" size="sm" onclick={(e) => { e.stopPropagation(); showAddMenu = !showAddMenu; positionAddMenu(); }} title={$t('home.addWidget')}>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
						<span class="hidden md:inline">{$t('home.addWidget')}</span>
					</Button>
					{#if showAddMenu}
						<div class="fixed z-[9999] bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] py-1.5 min-w-[220px] max-h-[400px] overflow-y-auto add-menu-enter"
							style={addMenuStyle}
							onclick={(e) => e.stopPropagation()}>
							<div class="px-3 py-1.5 text-[10px] text-[var(--text-muted)] uppercase tracking-[0.15em] font-semibold">{$t('home.widgets')}</div>
							{#each singletonTypes as sw}
								{#if !hasType(sw.type)}
									<button class="w-full flex items-center gap-2.5 px-3 py-2.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition-all text-left"
										onclick={() => addWidget(sw.type)}>
										<span class="text-sm">{sw.icon}</span>
										{sw.label}
									</button>
								{/if}
							{/each}
							<div class="border-t border-[var(--border)] my-1.5"></div>
							<div class="px-3 py-1.5 text-[10px] text-[var(--text-muted)] uppercase tracking-[0.15em] font-semibold">Custom</div>
							{#each customTypes as cw}
								<button class="w-full flex items-center gap-2.5 px-3 py-2.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition-all text-left"
									onclick={() => addWidget(cw.type)}>
									<span class="text-sm">{cw.icon}</span>
									{cw.label}
								</button>
							{/each}
							{#if availableServers.length > 0}
								<div class="border-t border-[var(--border)] my-1.5"></div>
								<div class="px-3 py-1.5 text-[10px] text-[var(--text-muted)] uppercase tracking-[0.15em] font-semibold">{$t('home.servers')}</div>
								{#each availableServers as s}
									<button class="w-full flex items-center gap-2.5 px-3 py-2.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition-all text-left"
										onclick={() => addWidget('server', s.id)}>
										<span class="w-2 h-2 rounded-full {s.info.status === 'online' ? 'bg-[var(--green)]' : 'bg-[var(--red)]'}"></span>
										{s.name}
									</button>
								{/each}
							{/if}
						</div>
					{/if}
				</div>
				<Button variant="secondary" size="sm" onclick={resetLayout} title={$t('home.resetLayout')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
					<span class="hidden md:inline">{$t('home.resetLayout')}</span>
				</Button>
				<Button variant="secondary" size="sm" onclick={handleExport} title={$t('dashboard.export')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
					<span class="hidden lg:inline">{$t('dashboard.export')}</span>
				</Button>
				<Button variant="secondary" size="sm" onclick={() => importFileEl?.click()} title={$t('dashboard.import')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
					<span class="hidden lg:inline">{$t('dashboard.import')}</span>
				</Button>
				<input type="file" accept=".json" class="hidden" bind:this={importFileEl} onchange={handleImport} />
				<Button variant="primary" size="sm" onclick={saveDashboard} title={$t('home.saveLayout')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
					<span class="hidden md:inline">{$t('home.saveLayout')}</span>
				</Button>
			{:else}
				<Button variant="secondary" size="sm" onclick={toggleEditMode} title={$t('home.editMode')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
					<span class="hidden md:inline">{$t('home.editMode')}</span>
				</Button>
			{/if}
		</div>
	</div>

	<!-- Edit mode banner -->
	{#if editMode}
		<div class="edit-banner mb-4 flex items-center gap-3 px-4 py-3 rounded-[var(--radius-lg)]">
			<div class="w-2 h-2 rounded-full bg-[var(--accent)] animate-pulse shrink-0"></div>
			<span class="text-xs text-[var(--accent)] font-medium">{$t('home.editBanner')}</span>
			{#if hasUnsavedChanges}
				<span class="ml-auto text-[10px] text-[var(--yellow)] font-medium px-2 py-0.5 rounded-full bg-[var(--yellow-bg)]">{$t('home.unsaved')}</span>
			{/if}
		</div>
	{/if}

	<!-- Tab Bar -->
	<div class="tab-bar flex items-center gap-1 px-2 py-1.5 mb-4 overflow-x-auto">
		{#each tabs as tab}
			{#if renamingTabId === tab.id}
				<input
					type="text"
					class="tab-item bg-[var(--bg-2)] border border-[var(--accent)] text-[var(--text)] text-[11px] px-2 py-1 rounded-[var(--radius)] w-28 focus:outline-none"
					bind:value={renameInput}
					onblur={finishRenameTab}
					onkeydown={(e) => { if (e.key === 'Enter') finishRenameTab(); if (e.key === 'Escape') { renamingTabId = null; } }}
				/>
			{:else}
				<button
					class="tab-item text-[var(--text)] {tab.id === activeTabId ? 'active' : ''} flex items-center gap-1.5"
					onclick={() => switchTab(tab.id)}
					ondblclick={() => { if (editMode) startRenameTab(tab.id, tab.name); }}
				>
					{tab.name}
					{#if editMode && tabs.length > 1}
						<span
							class="w-4 h-4 flex items-center justify-center rounded-full text-[var(--text-muted)] hover:text-[var(--red)] hover:bg-[var(--red-bg)] transition ml-1"
							onclick={(e) => { e.stopPropagation(); handleRemoveTab(tab.id); }}
						>
							<svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
						</span>
					{/if}
				</button>
			{/if}
		{/each}
		{#if editMode}
			<button
				class="tab-item text-[var(--text-muted)] hover:text-[var(--accent)] flex items-center gap-1"
				onclick={handleAddTab}
			>
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				{$t('dashboard.addTab')}
			</button>
		{/if}
	</div>

	<!-- Grid -->
	<div class="grid-stack {editMode ? 'edit-mode' : ''}" bind:this={gridEl}>
		{#each filteredWidgets as w (w.id)}
			<div class="grid-stack-item" id="widget-{w.id}" gs-id={w.id} gs-x={w.x} gs-y={w.y} gs-w={w.w} gs-h={w.h} gs-min-w="1" gs-min-h="2">
				<div class="grid-stack-item-content" style={w.color ? `border-left: 3px solid ${w.color}` : ''}>
					<!-- Widget Header -->
					<div class="widget-drag flex items-center justify-between px-3.5 py-2.5 border-b border-[var(--border)] shrink-0 {editMode ? 'cursor-grab active:cursor-grabbing bg-[var(--bg-3)]' : 'bg-transparent'}">
						<div class="flex items-center gap-2">
							{#if editMode}
								<svg class="w-3 h-3 text-[var(--text-muted)] opacity-50" viewBox="0 0 24 24" fill="currentColor">
									<circle cx="9" cy="5" r="1.5"/><circle cx="15" cy="5" r="1.5"/>
									<circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/>
									<circle cx="9" cy="19" r="1.5"/><circle cx="15" cy="19" r="1.5"/>
								</svg>
							{/if}
							<span class="text-[11px] font-semibold text-[var(--text)]">{widgetTitle(w)}</span>
						</div>
						{#if editMode}
							<div class="flex items-center gap-1.5">
								<div class="flex items-center gap-0.5">
									{#each widgetColors as c}
										<button
											class="color-dot {(w.color || '') === c.value ? 'selected' : ''}"
											style="background: {c.value || 'var(--accent)'};"
											title={c.name}
											onclick={(e) => { e.stopPropagation(); setWidgetColor(w.id, c.value); }}
										></button>
									{/each}
								</div>
								<button onclick={() => removeWidget(w.id)} title={$t('common.delete')}
									class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--text-muted)] hover:text-[var(--red)] hover:bg-[var(--red-bg)] transition-all">
									<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
								</button>
							</div>
						{/if}
					</div>
					<!-- Widget Content -->
					<div class="flex-1 overflow-y-auto">
						{#if w.type === 'server'}
							{@const srv = getServer(w.envId)}
							{#if srv}<ServerCardWidget server={srv} />{:else}<div class="p-4 text-xs text-[var(--text-muted)]">{$t('common.noResults')}</div>{/if}
						{:else if w.type === 'quick-stats'}
							<QuickStatsWidget {servers} />
						{:else if w.type === 'resources'}
							<ResourcesWidget {servers} />
						{:else if w.type === 'container-health'}
							<ContainerHealthWidget />
						{:else if w.type === 'stack-status'}
							<StackStatusWidget />
						{:else if w.type === 'disk-usage'}
							<DiskUsageWidget />
						{:else if w.type === 'uptime'}
							<UptimeWidget />
						{:else if w.type === 'unused-resources'}
							<UnusedResourcesWidget />
						{:else if w.type === 'quick-actions'}
							<QuickActionsWidget />
						{:else if w.type === 'resource-monitor'}
							<ResourceMonitorWidget />
						{:else if w.type === 'favorites'}
							<FavoritesWidget />
						{:else if w.type === 'note'}
							<NoteWidget widgetId={w.id} />
						{:else if w.type === 'links'}
							<LinksWidget widgetId={w.id} />
						{:else if w.type === 'clock'}
							<ClockWidget />
						{:else if w.type === 'iframe'}
							<IframeWidget widgetId={w.id} />
						{/if}
					</div>
				</div>
			</div>
		{/each}
	</div>

	{#if filteredWidgets.length === 0}
		<div class="empty-state rounded-[var(--radius-xl)] p-12 text-center mt-4">
			<svg class="w-14 h-14 mx-auto mb-4 opacity-20 text-[var(--text-secondary)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
				<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>
			</svg>
			<p class="text-sm font-medium text-[var(--text-secondary)]">{$t('home.noWidgets')}</p>
			<p class="text-xs text-[var(--text-muted)] mt-1.5 mb-4">{$t('home.noWidgetsDesc')}</p>
			<Button variant="primary" size="md" onclick={toggleEditMode}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				{$t('home.addWidgets')}
			</Button>
		</div>
	{/if}
{/if}

