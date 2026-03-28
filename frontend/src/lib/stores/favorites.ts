import { writable, get } from 'svelte/store';
import { browser } from '$app/environment';
import { auth } from './auth';

export interface FavoriteContainer {
	id: string;
	name: string;
	envId: string;
	image: string;
}

function storageKey(): string {
	const user = browser ? get(auth).username || 'default' : 'default';
	return `dp_favorites_${user}`;
}

function load(): FavoriteContainer[] {
	if (!browser) return [];
	try {
		const raw = localStorage.getItem(storageKey());
		if (raw) return JSON.parse(raw);
	} catch {}
	return [];
}

function save(favs: FavoriteContainer[]) {
	if (browser) localStorage.setItem(storageKey(), JSON.stringify(favs));
}

function createFavoritesStore() {
	const { subscribe, set } = writable<FavoriteContainer[]>(load());

	return {
		subscribe,

		add(container: FavoriteContainer) {
			const favs = load();
			if (favs.some(f => f.id === container.id)) return;
			favs.push(container);
			save(favs);
			set(favs);
		},

		remove(id: string) {
			const favs = load().filter(f => f.id !== id);
			save(favs);
			set(favs);
		},

		toggle(container: FavoriteContainer) {
			const favs = load();
			if (favs.some(f => f.id === container.id)) {
				const next = favs.filter(f => f.id !== container.id);
				save(next);
				set(next);
			} else {
				favs.push(container);
				save(favs);
				set(favs);
			}
		},

		isFavorite(id: string): boolean {
			return load().some(f => f.id === id);
		},
	};
}

export const favorites = createFavoritesStore();
