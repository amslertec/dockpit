import { writable, derived } from 'svelte/store';
import type { StatsSnapshot, ContainerStats } from '$lib/api/types';

function createStatsStore() {
	const { subscribe, set } = writable<StatsSnapshot | null>(null);
	let ws: WebSocket | null = null;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let currentEnvId = '';

	function connect(envId: string) {
		disconnect();
		currentEnvId = envId;
		const token = typeof window !== 'undefined' ? localStorage.getItem('dp_token') || '' : '';
		const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
		const url = `${proto}//${location.host}/api/env/${envId}/stats/live?token=${encodeURIComponent(token)}`;

		ws = new WebSocket(url);
		ws.onmessage = (event) => {
			try { set(JSON.parse(event.data)); } catch {}
		};
		ws.onclose = () => {
			ws = null;
			if (currentEnvId) {
				reconnectTimer = setTimeout(() => connect(currentEnvId), 3000);
			}
		};
		ws.onerror = () => { ws?.close(); };
	}

	function disconnect() {
		currentEnvId = '';
		if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
		if (ws) { ws.close(); ws = null; }
		set(null);
	}

	return { subscribe, connect, disconnect };
}

export const statsStore = createStatsStore();
export const currentStats = derived(statsStore, ($s) => $s?.containers || []);
