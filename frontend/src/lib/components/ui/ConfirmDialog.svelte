<script lang="ts">
	import Button from './Button.svelte';
	import { t } from '$lib/i18n';

	interface Props {
		title?: string;
		message: string;
		confirmText?: string;
		cancelText?: string;
		danger?: boolean;
		onconfirm: () => void;
		oncancel: () => void;
	}
	let { title = '', message, confirmText = '', cancelText = '', danger = true, onconfirm, oncancel }: Props = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1100] p-0 sm:p-4 confirm-backdrop-enter" onclick={(e) => { if (e.target === e.currentTarget) oncancel(); }}>
	<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-sm shadow-[var(--shadow-lg)] p-5 confirm-content-enter confirm-glass">
		<h3 class="text-[15px] font-semibold text-[var(--text)] mb-2">{title || $t('common.confirm')}</h3>
		<p class="text-sm text-[var(--text-secondary)] mb-5">{message}</p>
		<div class="flex justify-end gap-2">
			<Button variant="secondary" size="sm" onclick={oncancel}>{cancelText || $t('common.cancel')}</Button>
			<Button variant={danger ? 'danger' : 'primary'} size="sm" onclick={onconfirm}>{confirmText || $t('common.confirm')}</Button>
		</div>
	</div>
</div>

<style>
	.confirm-backdrop-enter {
		animation: backdrop-fade 0.2s ease-out;
	}
	.confirm-content-enter {
		animation: confirm-scale 0.25s ease-out;
	}
	@keyframes backdrop-fade {
		from { opacity: 0; }
		to { opacity: 1; }
	}
	@keyframes confirm-scale {
		from { opacity: 0; transform: translateY(16px) scale(0.97); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}
	@media (max-width: 639px) {
		@keyframes confirm-scale {
			from { opacity: 0; transform: translateY(100%); }
			to { opacity: 1; transform: translateY(0); }
		}
	}
	.confirm-glass {
		background: var(--glass-bg);
		backdrop-filter: blur(20px) saturate(150%);
		-webkit-backdrop-filter: blur(20px) saturate(150%);
	}
</style>
