import { writable } from 'svelte/store';
import { browser } from '$app/environment';

type Theme = 'dark' | 'light';

function createThemeStore() {
	const initial: Theme = browser
		? (localStorage.getItem('dp_theme') as Theme) || 'dark'
		: 'dark';

	const { subscribe, set } = writable<Theme>(initial);

	return {
		subscribe,
		toggle() {
			let newTheme: Theme;
			subscribe((t) => (newTheme = t === 'dark' ? 'light' : 'dark'))();
			if (browser) {
				localStorage.setItem('dp_theme', newTheme!);
				document.documentElement.classList.toggle('light', newTheme! === 'light');
			}
			set(newTheme!);
		},
		init() {
			if (browser) {
				const saved = localStorage.getItem('dp_theme') as Theme;
				if (saved === 'light') {
					document.documentElement.classList.add('light');
					set('light');
				}
			}
		}
	};
}

export const theme = createThemeStore();
