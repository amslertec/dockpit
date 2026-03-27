<script lang="ts">
	interface Props {
		checked?: boolean;
		onchange?: (checked: boolean) => void;
		label?: string;
		disabled?: boolean;
		size?: 'sm' | 'md';
	}
	let { checked = false, onchange, label, disabled = false, size = 'md' }: Props = $props();

	function toggle() {
		if (disabled) return;
		const next = !checked;
		onchange?.(next);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === ' ' || e.key === 'Enter') {
			e.preventDefault();
			toggle();
		}
	}

	const boxSize = $derived(size === 'sm' ? 'w-3.5 h-3.5' : 'w-4 h-4');
	const iconSize = $derived(size === 'sm' ? 'w-2.5 h-2.5' : 'w-3 h-3');
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<label
	class="inline-flex items-center gap-2.5 select-none {disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'} {label ? '' : 'inline-flex'}"
	onkeydown={onkeydown}
>
	<span
		class="{boxSize} shrink-0 rounded-[4px] border-2 flex items-center justify-center transition-all duration-200
		{checked
			? 'bg-[var(--accent)] border-[var(--accent)] shadow-[0_0_8px_var(--accent-bg)]'
			: 'border-[var(--border-light)] bg-transparent hover:border-[var(--accent)]'}"
		role="checkbox"
		aria-checked={checked}
		tabindex={disabled ? -1 : 0}
		onclick={toggle}
	>
		{#if checked}
			<svg class="{iconSize} text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
				<polyline points="20 6 9 17 4 12" class="check-animate" />
			</svg>
		{/if}
	</span>
	{#if label}
		<span class="text-sm text-[var(--text)]" onclick={toggle}>{label}</span>
	{/if}
</label>

<style>
	.check-animate {
		stroke-dasharray: 24;
		stroke-dashoffset: 24;
		animation: check-draw 0.2s ease forwards;
	}
	@keyframes check-draw {
		to { stroke-dashoffset: 0; }
	}
</style>
