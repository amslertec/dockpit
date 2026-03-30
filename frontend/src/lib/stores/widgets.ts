import { writable, get } from 'svelte/store';
import { browser } from '$app/environment';
import { auth } from './auth';
import { api } from '$lib/api/client';

export interface WidgetLink {
	title: string;
	url: string;
}

export interface WidgetConfig {
	id: string;
	type: 'server' | 'quick-stats' | 'resources' | 'container-health' | 'stack-status' | 'disk-usage' | 'uptime' | 'unused-resources' | 'quick-actions' | 'resource-monitor' | 'favorites' | 'note' | 'links' | 'clock' | 'iframe';
	envId?: string;
	x: number;
	y: number;
	w: number;
	h: number;
	// Extended fields
	content?: string;       // note widget text
	links?: WidgetLink[];   // links widget bookmarks
	iframeUrl?: string;     // iframe widget URL
	color?: string;         // widget theme color
	tabId?: string;         // which dashboard tab
}

export interface DashboardTab {
	id: string;
	name: string;
}

export interface DashboardData {
	tabs: DashboardTab[];
	widgets: WidgetConfig[];
}

function storageKey(): string {
	const user = browser ? get(auth).username || 'default' : 'default';
	return `dp_widgets_${user}`;
}

function loadData(): DashboardData {
	if (!browser) return { tabs: [{ id: 'default', name: 'Dashboard' }], widgets: [] };
	try {
		const raw = localStorage.getItem(storageKey());
		if (raw) {
			const parsed = JSON.parse(raw);
			// Migration: old format (array) → new format (object with tabs)
			if (Array.isArray(parsed)) {
				return {
					tabs: [{ id: 'default', name: 'Dashboard' }],
					widgets: parsed.map((w: WidgetConfig) => ({ ...w, tabId: w.tabId || 'default' })),
				};
			}
			// Ensure tabs exist
			if (!parsed.tabs || parsed.tabs.length === 0) {
				parsed.tabs = [{ id: 'default', name: 'Dashboard' }];
			}
			return parsed;
		}
	} catch {}
	return { tabs: [{ id: 'default', name: 'Dashboard' }], widgets: [] };
}

function saveData(data: DashboardData) {
	if (!browser) return;
	localStorage.setItem(storageKey(), JSON.stringify(data));
	// Persist to backend (fire-and-forget)
	const token = get(auth).token;
	if (token) {
		api.put('/dashboard-config', data).catch(() => {});
	}
}

/** Load dashboard config from backend and merge into localStorage */
export async function syncFromBackend(): Promise<boolean> {
	if (!browser) return false;
	const token = get(auth).token;
	if (!token) return false;
	try {
		const r = await api.get<string>('/dashboard-config');
		if (r.success && r.data && r.data.length > 0) {
			const data: DashboardData = JSON.parse(r.data);
			if (data.tabs && data.widgets) {
				localStorage.setItem(storageKey(), JSON.stringify(data));
				return true;
			}
		}
	} catch {}
	return false;
}

// Legacy compatibility
function loadWidgets(): WidgetConfig[] { return loadData().widgets; }
function saveWidgets(configs: WidgetConfig[]) {
	const data = loadData();
	data.widgets = configs;
	saveData(data);
}

function createWidgetStore() {
	const initial = loadData();
	const { subscribe, set } = writable<WidgetConfig[]>(initial.widgets);

	function persist(configs: WidgetConfig[]) {
		saveWidgets(configs);
		set(configs);
	}

	return {
		subscribe,

		/** Reload store from localStorage (e.g. after syncFromBackend) */
		reload() {
			set(loadData().widgets);
		},

		init(envIds: string[]) {
			const existing = loadWidgets();
			if (existing.length > 0) {
				if (envIds.length > 0) {
					const valid = existing.filter(w => w.type !== 'server' || envIds.includes(w.envId || ''));
					persist(valid);
				}
				return;
			}
			const configs: WidgetConfig[] = [
				{ id: 'quick-stats', type: 'quick-stats', x: 0, y: 0, w: 6, h: 2, tabId: 'default' },
				{ id: 'container-health', type: 'container-health', x: 0, y: 2, w: 2, h: 3, tabId: 'default' },
				{ id: 'stack-status', type: 'stack-status', x: 2, y: 2, w: 2, h: 3, tabId: 'default' },
				...envIds.map((envId, i) => ({
					id: `server-${envId}`, type: 'server' as const, envId,
					x: (i % 3) * 2, y: 5 + Math.floor(i / 3) * 3, w: 2, h: 3, tabId: 'default',
				})),
				{ id: 'disk-usage', type: 'disk-usage', x: 0, y: 20, w: 2, h: 3, tabId: 'default' },
				{ id: 'resources', type: 'resources', x: 2, y: 20, w: 2, h: 3, tabId: 'default' },
				{ id: 'unused-resources', type: 'unused-resources', x: 4, y: 20, w: 2, h: 3, tabId: 'default' },
			];
			persist(configs);
		},

		addWidget(type: WidgetConfig['type'], envId?: string, tabId?: string) {
			const configs = loadWidgets();
			const tid = tabId || loadData().tabs[0]?.id || 'default';
			const tabWidgets = configs.filter(w => (w.tabId || 'default') === tid);
			const id = type === 'server' ? `server-${envId}` : `${type}-${Date.now()}`;
			const maxY = tabWidgets.reduce((max, w) => Math.max(max, w.y + w.h), 0);
			configs.push({ id, type, envId, x: 0, y: maxY, w: 2, h: 3, tabId: tid });
			persist(configs);
		},

		removeWidget(id: string) {
			persist(loadWidgets().filter(w => w.id !== id));
		},

		updateWidget(id: string, updates: Partial<WidgetConfig>) {
			const configs = loadWidgets();
			const w = configs.find(c => c.id === id);
			if (w) Object.assign(w, updates);
			persist(configs);
		},

		updateLayout(items: { id: string; x: number; y: number; w: number; h: number }[]) {
			const configs = loadWidgets();
			for (const item of items) {
				const w = configs.find(c => c.id === item.id);
				if (w) { w.x = item.x; w.y = item.y; w.w = item.w; w.h = item.h; }
			}
			persist(configs);
		},

		reset(envIds: string[]) {
			if (browser) localStorage.removeItem(storageKey());
			const configs: WidgetConfig[] = [
				{ id: 'quick-stats', type: 'quick-stats', x: 0, y: 0, w: 6, h: 2, tabId: 'default' },
				{ id: 'container-health', type: 'container-health', x: 0, y: 2, w: 2, h: 3, tabId: 'default' },
				{ id: 'stack-status', type: 'stack-status', x: 2, y: 2, w: 2, h: 3, tabId: 'default' },
				...envIds.map((envId, i) => ({
					id: `server-${envId}`, type: 'server' as const, envId,
					x: (i % 3) * 2, y: 5 + Math.floor(i / 3) * 3, w: 2, h: 3, tabId: 'default',
				})),
				{ id: 'disk-usage', type: 'disk-usage', x: 0, y: 20, w: 2, h: 3, tabId: 'default' },
				{ id: 'resources', type: 'resources', x: 2, y: 20, w: 2, h: 3, tabId: 'default' },
				{ id: 'unused-resources', type: 'unused-resources', x: 4, y: 20, w: 2, h: 3, tabId: 'default' },
			];
			const data: DashboardData = { tabs: [{ id: 'default', name: 'Dashboard' }], widgets: configs };
			saveData(data);
			set(configs);
		},
	};
}

// === Tab Management ===
export function getTabs(): DashboardTab[] { return loadData().tabs; }

export function addTab(name: string): DashboardTab {
	const data = loadData();
	const tab: DashboardTab = { id: `tab-${Date.now()}`, name };
	data.tabs.push(tab);
	saveData(data);
	return tab;
}

export function renameTab(id: string, name: string) {
	const data = loadData();
	const tab = data.tabs.find(t => t.id === id);
	if (tab) { tab.name = name; saveData(data); }
}

export function removeTab(id: string) {
	const data = loadData();
	if (data.tabs.length <= 1) return; // can't remove last tab
	data.tabs = data.tabs.filter(t => t.id !== id);
	data.widgets = data.widgets.filter(w => (w.tabId || 'default') !== id);
	saveData(data);
}

// === Import/Export ===
export function exportDashboard(): string {
	return JSON.stringify(loadData(), null, 2);
}

export function importDashboard(json: string): boolean {
	try {
		const data: DashboardData = JSON.parse(json);
		if (!data.tabs || !data.widgets) return false;
		saveData(data);
		return true;
	} catch { return false; }
}

export const widgets = createWidgetStore();
