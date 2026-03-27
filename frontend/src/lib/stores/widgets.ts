import { writable, get } from 'svelte/store';
import { browser } from '$app/environment';
import { auth } from './auth';

export interface WidgetConfig {
	id: string;
	type: 'server' | 'quick-stats' | 'resources' | 'container-health' | 'stack-status' | 'disk-usage' | 'uptime' | 'unused-resources' | 'quick-actions' | 'resource-monitor';
	envId?: string;
	x: number;
	y: number;
	w: number;
	h: number;
}

function storageKey(): string {
	const user = browser ? get(auth).username || 'default' : 'default';
	return `dp_widgets_${user}`;
}

function loadWidgets(): WidgetConfig[] {
	if (!browser) return [];
	try {
		const raw = localStorage.getItem(storageKey());
		if (raw) return JSON.parse(raw);
	} catch {}
	return [];
}

function saveWidgets(configs: WidgetConfig[]) {
	if (browser) localStorage.setItem(storageKey(), JSON.stringify(configs));
}

function createWidgetStore() {
	const { subscribe, set } = writable<WidgetConfig[]>(loadWidgets());

	function persist(configs: WidgetConfig[]) {
		saveWidgets(configs);
		set(configs);
	}

	return {
		subscribe,

		init(envIds: string[]) {
			const existing = loadWidgets();
			if (existing.length > 0) {
				// Only filter out stale server widgets if we have env data
				if (envIds.length > 0) {
					const valid = existing.filter(w => w.type !== 'server' || envIds.includes(w.envId || ''));
					persist(valid);
				}
				return;
			}
			// Default layout: 6 columns
			const configs: WidgetConfig[] = [
				{ id: 'quick-stats', type: 'quick-stats', x: 0, y: 0, w: 6, h: 2 },
				{ id: 'container-health', type: 'container-health', x: 0, y: 2, w: 2, h: 3 },
				{ id: 'stack-status', type: 'stack-status', x: 2, y: 2, w: 2, h: 3 },
				...envIds.map((envId, i) => ({
					id: `server-${envId}`,
					type: 'server' as const,
					envId,
					x: (i % 3) * 2, y: 5 + Math.floor(i / 3) * 3, w: 2, h: 3,
				})),
				{ id: 'disk-usage', type: 'disk-usage', x: 0, y: 20, w: 2, h: 3 },
				{ id: 'resources', type: 'resources', x: 2, y: 20, w: 2, h: 3 },
				{ id: 'unused-resources', type: 'unused-resources', x: 4, y: 20, w: 2, h: 3 },
			];
			persist(configs);
		},

		addWidget(type: WidgetConfig['type'], envId?: string) {
			const configs = loadWidgets();
			const id = type === 'server' ? `server-${envId}` : `${type}-${Date.now()}`;
			// Find next free y position
			const maxY = configs.reduce((max, w) => Math.max(max, w.y + w.h), 0);
			configs.push({ id, type, envId, x: 0, y: maxY, w: 2, h: 3 });
			persist(configs);
		},

		removeWidget(id: string) {
			const configs = loadWidgets().filter(w => w.id !== id);
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
			// Recreate default layout
			const configs: WidgetConfig[] = [
				{ id: 'quick-stats', type: 'quick-stats', x: 0, y: 0, w: 6, h: 2 },
				{ id: 'container-health', type: 'container-health', x: 0, y: 2, w: 2, h: 3 },
				{ id: 'stack-status', type: 'stack-status', x: 2, y: 2, w: 2, h: 3 },
				...envIds.map((envId, i) => ({
					id: `server-${envId}`,
					type: 'server' as const,
					envId,
					x: (i % 3) * 2, y: 5 + Math.floor(i / 3) * 3, w: 2, h: 3,
				})),
				{ id: 'disk-usage', type: 'disk-usage', x: 0, y: 20, w: 2, h: 3 },
				{ id: 'resources', type: 'resources', x: 2, y: 20, w: 2, h: 3 },
				{ id: 'unused-resources', type: 'unused-resources', x: 4, y: 20, w: 2, h: 3 },
			];
			persist(configs);
		},
	};
}

export const widgets = createWidgetStore();
