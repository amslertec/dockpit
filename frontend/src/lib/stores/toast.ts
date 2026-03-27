import { writable } from 'svelte/store';

export interface ToastMessage {
	id: number;
	text: string;
	type: 'success' | 'error';
}

let nextId = 0;

function createToastStore() {
	const { subscribe, update } = writable<ToastMessage[]>([]);

	return {
		subscribe,
		success(text: string) {
			const id = nextId++;
			update((t) => [...t, { id, text, type: 'success' }]);
			setTimeout(() => update((t) => t.filter((m) => m.id !== id)), 3500);
		},
		error(text: string) {
			const id = nextId++;
			update((t) => [...t, { id, text, type: 'error' }]);
			setTimeout(() => update((t) => t.filter((m) => m.id !== id)), 4500);
		},
		dismiss(id: number) {
			update((t) => t.filter((m) => m.id !== id));
		}
	};
}

export const toasts = createToastStore();
