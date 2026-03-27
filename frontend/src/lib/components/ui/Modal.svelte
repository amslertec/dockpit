<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		onclose: () => void;
		children: Snippet;
		footer?: Snippet;
	}
	let { title, onclose, children, footer }: Props = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1000] p-0 sm:p-4 modal-backdrop-enter" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
	<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-lg max-h-[90vh] sm:max-h-[85vh] overflow-y-auto shadow-[var(--shadow-lg)] modal-content-enter modal-glass">
		<div class="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
			<h3 class="text-[15px] font-semibold text-[var(--text)]">{title}</h3>
			<button class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all duration-200" onclick={onclose}>
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
			</button>
		</div>
		<div class="p-5">
			{@render children()}
		</div>
		{#if footer}
			<div class="px-5 py-3 border-t border-[var(--border)] flex justify-end gap-2">
				{@render footer()}
			</div>
		{/if}
	</div>
</div>

<style>
	.modal-backdrop-enter {
		animation: backdrop-fade 0.2s ease-out;
	}
	.modal-content-enter {
		animation: modal-scale 0.25s ease-out;
	}
	@keyframes backdrop-fade {
		from { opacity: 0; }
		to { opacity: 1; }
	}
	@keyframes modal-scale {
		from { opacity: 0; transform: translateY(16px) scale(0.97); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}
	@media (max-width: 639px) {
		@keyframes modal-scale {
			from { opacity: 0; transform: translateY(100%); }
			to { opacity: 1; transform: translateY(0); }
		}
	}
	.modal-glass {
		background: var(--glass-bg);
		backdrop-filter: blur(20px) saturate(150%);
		-webkit-backdrop-filter: blur(20px) saturate(150%);
	}
</style>
