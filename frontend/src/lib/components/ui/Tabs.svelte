<script lang="ts">
	interface Tab {
		id: number | string;
		label: string;
	}
	interface Props {
		tabs: Tab[];
		active: number | string;
		onchange: (id: number | string) => void;
	}
	let { tabs, active, onchange }: Props = $props();

	let tabEls: HTMLButtonElement[] = $state([]);
	let indicatorStyle = $state('');

	function updateIndicator() {
		const idx = tabs.findIndex(t => t.id === active);
		if (idx >= 0 && tabEls[idx]) {
			const el = tabEls[idx];
			indicatorStyle = `left: ${el.offsetLeft}px; width: ${el.offsetWidth}px;`;
		}
	}

	$effect(() => {
		active;
		requestAnimationFrame(updateIndicator);
	});
</script>

<div class="relative flex border-b border-[var(--border)] overflow-x-auto scrollbar-none">
	{#each tabs as tab, i}
		<button
			bind:this={tabEls[i]}
			class="px-5 py-3 text-xs font-medium transition-colors duration-200 relative z-10 whitespace-nowrap shrink-0
			{active === tab.id ? 'text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:text-[var(--text)]'}"
			onclick={() => onchange(tab.id)}
		>
			{tab.label}
		</button>
	{/each}
	<div
		class="absolute bottom-0 h-[2px] bg-[var(--accent)] transition-all duration-300 ease-out rounded-full"
		style={indicatorStyle}
	></div>
</div>

<style>
	.scrollbar-none { -ms-overflow-style: none; scrollbar-width: none; }
	.scrollbar-none::-webkit-scrollbar { display: none; }
</style>
