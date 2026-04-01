import { writable, derived, get } from 'svelte/store';
import { browser } from '$app/environment';

interface AuthState {
	token: string | null;
	username: string | null;
	role: string;
	permissions: string[];
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
	const savedPerms = browser ? localStorage.getItem('dp_permissions') : null;
	const initial: AuthState = {
		token,
		username: browser ? localStorage.getItem('dp_user') : null,
		role: parseRole(token),
		permissions: savedPerms ? JSON.parse(savedPerms) : [],
	};

	const { subscribe, set, update } = writable<AuthState>(initial);

	return {
		subscribe,
		login(token: string, username: string) {
			if (browser) {
				localStorage.setItem('dp_token', token);
				localStorage.setItem('dp_user', username);
			}
			set({ token, username, role: parseRole(token), permissions: [] });
		},
		logout() {
			if (browser) {
				localStorage.removeItem('dp_token');
				localStorage.removeItem('dp_user');
				localStorage.removeItem('dp_permissions');
			}
			set({ token: null, username: null, role: 'viewer', permissions: [] });
		},
		setPermissions(perms: string[]) {
			if (browser) localStorage.setItem('dp_permissions', JSON.stringify(perms));
			update(s => ({ ...s, permissions: perms }));
		}
	};
}

export const auth = createAuthStore();

// Permission check helper
export function hasPerm(perms: string[], perm: string): boolean {
	if (perms.includes('*')) return true;
	return perms.includes(perm);
}

// Derived permission helpers
export const canManageUsers = derived(auth, $a => $a.role === 'super_admin' || hasPerm($a.permissions, 'action.user_management'));
export const canManageDocker = derived(auth, $a => $a.role === 'super_admin' || hasPerm($a.permissions, 'action.container_recreate') || hasPerm($a.permissions, 'action.backup'));
export const canEditContainers = derived(auth, $a => $a.role === 'super_admin' || hasPerm($a.permissions, 'action.container_start_stop'));
export const isViewer = derived(auth, $a => $a.role === 'viewer' && $a.permissions.length === 0);

// New permission-based derived stores
export const userPermissions = derived(auth, $a => $a.permissions);
export const canSeePage = derived(auth, $a => {
	return (page: string): boolean => {
		// super_admin always has full access
		if ($a.role === 'super_admin') return true;
		// All other roles: check group permissions
		return hasPerm($a.permissions, page);
	};
});

export const canDoAction = derived(auth, $a => {
	return (action: string): boolean => {
		// super_admin always has full access
		if ($a.role === 'super_admin') return true;
		// All other roles: check group permissions
		return hasPerm($a.permissions, action);
	};
});
