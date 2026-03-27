import { writable, derived, get } from 'svelte/store';
import { browser } from '$app/environment';
import { en } from './en';
import { de } from './de';

export type Locale = 'en' | 'de';

const translations: Record<Locale, Record<string, string>> = { en, de };

function createLocaleStore() {
	const saved = browser ? (localStorage.getItem('dp_locale') as Locale) || 'en' : 'en';
	const { subscribe, set } = writable<Locale>(saved);

	return {
		subscribe,
		set(locale: Locale) {
			if (browser) localStorage.setItem('dp_locale', locale);
			set(locale);
		},
		toggle() {
			const current = get({ subscribe });
			const next: Locale = current === 'en' ? 'de' : 'en';
			if (browser) localStorage.setItem('dp_locale', next);
			set(next);
		}
	};
}

export const locale = createLocaleStore();

export const t = derived(locale, ($locale) => {
	const dict = translations[$locale] || translations.en;
	return (key: string, params?: Record<string, string | number>): string => {
		let str = dict[key] || translations.en[key] || key;
		if (params) {
			for (const [k, v] of Object.entries(params)) {
				str = str.replace(`{${k}}`, String(v));
			}
		}
		return str;
	};
});
