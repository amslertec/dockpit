import { writable } from 'svelte/store';
import { browser } from '$app/environment';
import type { EnvironmentInfo } from '$lib/api/types';

function createEnvStore() {
	const initial = browser ? localStorage.getItem('dp_env') || '' : '';
	const { subscribe, set } = writable<string>(initial);

	return {
		subscribe,
		select(id: string) {
			if (browser) localStorage.setItem('dp_env', id);
			set(id);
		}
	};
}

function loadCachedEnvironments(): EnvironmentInfo[] {
	if (!browser) return [];
	try {
		const raw = localStorage.getItem('dp_environments');
		if (raw) return JSON.parse(raw);
	} catch {}
	return [];
}

function createEnvironmentsStore() {
	const { subscribe, set: _set } = writable<EnvironmentInfo[]>(loadCachedEnvironments());

	return {
		subscribe,
		set(envs: EnvironmentInfo[]) {
			if (browser) localStorage.setItem('dp_environments', JSON.stringify(envs));
			_set(envs);
		},
		update(fn: (envs: EnvironmentInfo[]) => EnvironmentInfo[]) {
			let current: EnvironmentInfo[] = [];
			const unsub = subscribe(v => { current = v; });
			unsub();
			const next = fn(current);
			if (browser) localStorage.setItem('dp_environments', JSON.stringify(next));
			_set(next);
		}
	};
}

export const selectedEnv = createEnvStore();
export const environments = createEnvironmentsStore();
