<script lang="ts">
	import { toasts } from '$lib/stores/toast';
</script>

<div class="fixed top-3 right-3 z-[2000] flex flex-col gap-2 max-w-[380px]">
	{#each $toasts as msg (msg.id)}
		<div
			class="bg-[var(--glass-bg)] backdrop-blur-xl border border-[var(--glass-border)] rounded-[var(--radius-lg)] px-4 py-3 text-sm shadow-[var(--shadow-lg)] flex items-start gap-3 toast-enter
			{msg.type === 'success' ? 'border-l-[3px] border-l-[var(--green)]' : 'border-l-[3px] border-l-[var(--red)]'}"
		>
			<div class="shrink-0 mt-0.5">
				{#if msg.type === 'success'}
					<div class="w-5 h-5 rounded-full bg-[var(--green)] flex items-center justify-center">
						<svg class="w-3 h-3 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
					</div>
				{:else}
					<div class="w-5 h-5 rounded-full bg-[var(--red)] flex items-center justify-center">
						<svg class="w-3 h-3 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					</div>
				{/if}
			</div>
			<span class="text-[var(--text)] flex-1 text-[13px] leading-snug">{msg.text}</span>
			<button class="shrink-0 text-[var(--text-muted)] hover:text-[var(--text)] transition-colors" onclick={() => toasts.dismiss(msg.id)}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
			</button>
		</div>
	{/each}
</div>

<style>
	.toast-enter {
		animation: toast-slide 0.25s ease-out;
	}
	@keyframes toast-slide {
		from { transform: translateX(100%) translateY(-8px); opacity: 0; }
		to { transform: translateX(0) translateY(0); opacity: 1; }
	}
</style>
