<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { statsStore, currentStats } from '$lib/stores/stats';
	import { environments } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import type { ContainerStats } from '$lib/api/types';

	function formatBytes(b: number): string {
		if (b < 1024) return b + ' B';
		if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
		if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
		return (b / 1073741824).toFixed(1) + ' GB';
	}

	function barColor(pct: number): string {
		if (pct < 50) return 'var(--green)';
		if (pct < 80) return 'var(--yellow)';
		return 'var(--red)';
	}

	const top5 = $derived(
		[...($currentStats || [])]
			.sort((a: ContainerStats, b: ContainerStats) => b.cpu_percent - a.cpu_percent)
			.slice(0, 5)
	);

	onMount(() => {
		const envs = $environments;
		const online = envs.find(e => e.status === 'online' || e.is_local);
		if (online) statsStore.connect(online.id);
	});

	onDestroy(() => {
		statsStore.disconnect();
	});
</script>

<div class="flex flex-col h-full">
	<div class="flex-1 space-y-2">
		{#if top5.length === 0}
			<div class="flex items-center justify-center h-full text-[var(--text-muted)] text-xs">
				{$t('monitoring.noContainers')}
			</div>
		{:else}
			{#each top5 as c (c.id)}
				<div class="space-y-1">
					<div class="flex items-center justify-between text-xs">
						<span class="text-[var(--text)] font-medium truncate max-w-[120px]">{c.name}</span>
						<span class="text-[var(--text-muted)] tabular-nums">{c.cpu_percent.toFixed(1)}%</span>
					</div>
					<div class="flex gap-1.5">
						<div class="flex-1 h-1.5 rounded-full bg-[var(--bg-2)] overflow-hidden" title="{$t('monitoring.cpu')}: {c.cpu_percent.toFixed(1)}%">
							<div class="h-full rounded-full transition-all duration-500" style="width: {Math.min(c.cpu_percent, 100)}%; background: {barColor(c.cpu_percent)}"></div>
						</div>
						<div class="flex-1 h-1.5 rounded-full bg-[var(--bg-2)] overflow-hidden" title="{$t('monitoring.memory')}: {c.memory_percent.toFixed(1)}%">
							<div class="h-full rounded-full transition-all duration-500" style="width: {Math.min(c.memory_percent, 100)}%; background: {barColor(c.memory_percent)}"></div>
						</div>
					</div>
					<div class="flex justify-between text-[10px] text-[var(--text-muted)]">
						<span>{$t('monitoring.cpu')}</span>
						<span>{$t('monitoring.memory')} {formatBytes(c.memory_usage)}</span>
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<a href="/monitoring" class="mt-2 text-xs text-[var(--accent)] hover:underline text-center block">
		{$t('monitoring.viewAll')}
	</a>
</div>
