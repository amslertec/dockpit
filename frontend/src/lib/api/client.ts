import { auth } from '$lib/stores/auth';
import { get } from 'svelte/store';
import type { ApiResponse } from './types';

async function request<T>(path: string, opts: RequestInit = {}): Promise<ApiResponse<T>> {
	const token = get(auth).token || (typeof localStorage !== 'undefined' ? localStorage.getItem('dp_token') : null);
	const headers: Record<string, string> = { 'Content-Type': 'application/json' };
	if (token) headers['Authorization'] = `Bearer ${token}`;

	try {
		const res = await fetch(`/api${path}`, { ...opts, headers });
		if (res.status === 401) {
			// Try to refresh the token once
			if (token && path !== '/refresh') {
				const refreshRes = await fetch('/api/refresh', {
					method: 'POST',
					headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${token}` },
				});
				if (refreshRes.ok) {
					try {
						const refreshData = JSON.parse(await refreshRes.text());
						if (refreshData.success && refreshData.data?.token) {
							auth.login(refreshData.data.token, refreshData.data.username);
							// Retry original request with new token
							headers['Authorization'] = `Bearer ${refreshData.data.token}`;
							const retry = await fetch(`/api${path}`, { ...opts, headers });
							const retryText = await retry.text();
							if (!retryText) return retry.ok ? { success: true } as ApiResponse<T> : { success: false, error: `HTTP ${retry.status}` };
							try { return JSON.parse(retryText); } catch { return retry.ok ? { success: true } as ApiResponse<T> : { success: false, error: retryText }; }
						}
					} catch {}
				}
			}
			// Refresh failed — logout
			auth.logout();
			if (typeof window !== 'undefined'
				&& !window.location.pathname.startsWith('/login')
				&& !window.location.pathname.startsWith('/setup')) {
				window.location.href = '/login';
			}
			return { success: false, error: 'Sitzung abgelaufen' };
		}
		if (res.status === 403) {
			return { success: false, error: 'Keine Berechtigung für diese Aktion' };
		}
		if (res.status === 404) {
			return { success: false, error: 'Nicht gefunden' };
		}
		// Try to parse JSON, handle empty responses
		const text = await res.text();
		if (!text) {
			return res.ok ? { success: true } as ApiResponse<T> : { success: false, error: `HTTP ${res.status}` };
		}
		try {
			return JSON.parse(text);
		} catch {
			return res.ok ? { success: true } as ApiResponse<T> : { success: false, error: text || `HTTP ${res.status}` };
		}
	} catch (e) {
		return { success: false, error: (e as Error).message };
	}
}

export const api = {
	get: <T>(path: string) => request<T>(path),
	post: <T>(path: string, body: unknown) =>
		request<T>(path, { method: 'POST', body: JSON.stringify(body) }),
	put: <T>(path: string, body: unknown) =>
		request<T>(path, { method: 'PUT', body: JSON.stringify(body) }),
	del: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
	/** Get a one-time WebSocket token (valid 30s, single use) */
	getWsToken: async (): Promise<string> => {
		const r = await request<string>('/ws-token', { method: 'POST', body: '{}' });
		return r.success && r.data ? r.data : (get(auth).token || '');
	}
};
