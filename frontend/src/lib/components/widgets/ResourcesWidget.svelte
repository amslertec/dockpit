<script lang="ts">
	import type { ServerOverview } from '$lib/api/types';
	import { t } from '$lib/i18n';

	interface Props { servers: ServerOverview[]; }
	let { servers }: Props = $props();

	const online = $derived(servers.filter(s => s.info.status === 'online'));
	const totalCpus = $derived(online.reduce((a, s) => a + s.info.cpus, 0));
	const totalMemGB = $derived(online.reduce((a, s) => a + s.info.memory_bytes, 0) / 1_073_741_824);
</script>

<div class="p-4">
	<!-- Totals -->
	<div class="grid grid-cols-2 gap-4 mb-4">
		<div class="text-center">
			<div class="text-2xl font-bold text-accent">{totalCpus}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('widget.cpuCores')}</div>
		</div>
		<div class="text-center">
			<div class="text-2xl font-bold text-accent">{totalMemGB.toFixed(1)} GB</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('widget.ramTotal')}</div>
		</div>
	</div>

	<!-- Per server -->
	{#if online.length > 0}
		<div class="space-y-2">
			{#each online as s}
				<div class="flex items-center justify-between text-xs">
					<span class="text-secondary truncate flex-1">{s.name}</span>
					<div class="flex items-center gap-3 shrink-0 text-[11px]">
						<span class="text-muted">{s.info.cpus} {$t('widget.cpu')}</span>
						<span class="text-muted">{s.info.memory_display}</span>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
