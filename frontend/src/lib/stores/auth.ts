import { writable, derived } from 'svelte/store';
import { browser } from '$app/environment';

interface AuthState {
	token: string | null;
	username: string | null;
	role: string;
}

function parseRole(token: string | null): string {
	if (!token) return 'viewer';
	try {
		const payload = JSON.parse(atob(token.split('.')[1]));
		return payload.role || 'admin';
	} catch {
		return 'admin';
	}
}

function createAuthStore() {
	const token = browser ? localStorage.getItem('dp_token') : null;
	const initial: AuthState = {
		token,
		username: browser ? localStorage.getItem('dp_user') : null,
		role: parseRole(token),
	};

	const { subscribe, set } = writable<AuthState>(initial);

	return {
		subscribe,
		login(token: string, username: string) {
			if (browser) {
				localStorage.setItem('dp_token', token);
				localStorage.setItem('dp_user', username);
			}
			set({ token, username, role: parseRole(token) });
		},
		logout() {
			if (browser) {
				localStorage.removeItem('dp_token');
				localStorage.removeItem('dp_user');
			}
			set({ token: null, username: null, role: 'viewer' });
		}
	};
}

export const auth = createAuthStore();

// Derived permission helpers
export const canManageUsers = derived(auth, $a => $a.role === 'super_admin');
export const canManageDocker = derived(auth, $a => ['super_admin', 'admin'].includes($a.role));
export const canEditContainers = derived(auth, $a => ['super_admin', 'admin', 'editor'].includes($a.role));
export const isViewer = derived(auth, $a => $a.role === 'viewer');
