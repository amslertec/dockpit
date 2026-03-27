<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import { formatSize } from '$lib/utils/format';
	import type { DiskUsageInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	let data = $state<{ name: string; usage: DiskUsageInfo }[]>([]);
	let loading = $state(true);

	onMount(async () => {
		for (const env of $environments) {
			const r = await api.get<DiskUsageInfo>(`/env/${env.id}/disk-usage`);
			if (r.success && r.data) data.push({ name: env.name, usage: r.data });
		}
		loading = false;
	});

	const totalSize = $derived(data.reduce((a, d) => a + d.usage.total_size, 0));

	function pct(val: number, total: number): string {
		return total > 0 ? (val / total * 100).toFixed(1) : '0';
	}
</script>

<div class="p-4">
	{#if loading}
		<div class="flex justify-center py-4"><div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="text-center mb-4">
			<div class="text-2xl font-bold text-primary">{formatSize(totalSize)}</div>
			<div class="text-[10px] text-muted">{$t('widget.totalDockerStorage')}</div>
		</div>

		{#each data as d}
			<div class="mb-3 last:mb-0">
				<div class="text-xs font-medium text-secondary mb-1.5">{d.name}</div>
				<div class="grid grid-cols-2 gap-x-4 gap-y-1 text-[11px]">
					<div class="flex justify-between"><span class="text-muted">Images</span><span class="text-primary">{formatSize(d.usage.images_size)}</span></div>
					<div class="flex justify-between"><span class="text-muted">Container</span><span class="text-primary">{formatSize(d.usage.containers_size)}</span></div>
					<div class="flex justify-between"><span class="text-muted">Volumes</span><span class="text-primary">{formatSize(d.usage.volumes_size)}</span></div>
					<div class="flex justify-between"><span class="text-muted">Build Cache</span><span class="text-primary">{formatSize(d.usage.build_cache_size)}</span></div>
				</div>
				<div class="flex h-1.5 rounded-full overflow-hidden bg-0 mt-2">
					<div class="bg-[var(--purple)]" style="width:{pct(d.usage.images_size, d.usage.total_size)}%"></div>
					<div class="bg-[var(--accent)]" style="width:{pct(d.usage.containers_size, d.usage.total_size)}%"></div>
					<div class="bg-[var(--yellow)]" style="width:{pct(d.usage.volumes_size, d.usage.total_size)}%"></div>
					<div class="bg-[var(--green)]" style="width:{pct(d.usage.build_cache_size, d.usage.total_size)}%"></div>
				</div>
			</div>
		{/each}
	{/if}
</div>
