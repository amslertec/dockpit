<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		colSpan: 1 | 2 | 3;
		onremove: () => void;
		onresize?: (span: 1 | 2 | 3) => void;
		draggable?: boolean;
		children: Snippet;
	}
	let { title, colSpan, onremove, onresize, draggable = true, children }: Props = $props();

	let showMenu = $state(false);

	const spanCls: Record<number, string> = { 1: 'col-span-1', 2: 'md:col-span-2', 3: 'md:col-span-3 lg:col-span-3' };
</script>

<div class="{spanCls[colSpan] || 'col-span-1'} bg-card border border-theme rounded-xl overflow-hidden transition hover:border-light group"
	draggable={draggable ? 'true' : undefined}
	role="listitem">

	<!-- Header -->
	<div class="flex items-center justify-between px-4 py-2.5 border-b border-theme bg-1 cursor-grab active:cursor-grabbing">
		<div class="flex items-center gap-2">
			<svg class="w-3 h-3 text-muted opacity-0 group-hover:opacity-50 transition" viewBox="0 0 24 24" fill="currentColor">
				<circle cx="9" cy="5" r="1.5"/><circle cx="15" cy="5" r="1.5"/>
				<circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/>
				<circle cx="9" cy="19" r="1.5"/><circle cx="15" cy="19" r="1.5"/>
			</svg>
			<h3 class="text-xs font-semibold text-primary">{title}</h3>
		</div>
		<div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition">
			<!-- Resize -->
			{#if onresize}
				<div class="relative">
					<button onclick={() => showMenu = !showMenu}
						class="w-6 h-6 flex items-center justify-center rounded text-muted hover:text-primary transition">
						<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/>
						</svg>
					</button>
					{#if showMenu}
						<div class="absolute right-0 top-full mt-1 bg-card border border-theme rounded-lg shadow-xl z-50 py-1 min-w-[100px]">
							{#each [1, 2, 3] as span}
								<button onclick={() => { onresize(span as 1|2|3); showMenu = false; }}
									class="w-full px-3 py-1.5 text-[11px] text-left transition
									{colSpan === span ? 'text-accent font-medium' : 'text-secondary hover:text-primary hover:bg-hover'}">
									{span === 1 ? 'Klein' : span === 2 ? 'Mittel' : 'Gross'}
								</button>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
			<!-- Remove -->
			<button onclick={onremove} title="Widget entfernen"
				class="w-6 h-6 flex items-center justify-center rounded text-muted hover:text-red transition">
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
			</button>
		</div>
	</div>

	<!-- Content -->
	<div>
		{@render children()}
	</div>
</div>

<svelte:window onclick={() => showMenu = false} />
