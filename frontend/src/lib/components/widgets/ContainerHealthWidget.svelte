<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { extractHealth } from '$lib/utils/format';
	import type { ContainerInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	let problems = $state<(ContainerInfo & { serverName: string })[]>([]);
	let loading = $state(true);

	onMount(async () => {
		for (const env of $environments) {
			const r = await api.get<ContainerInfo[]>(`/env/${env.id}/containers`);
			if (r.success && r.data) {
				for (const c of r.data) {
					const health = extractHealth(c.status);
					if (health === 'unhealthy' || c.state === 'restarting' || c.state === 'dead' || (c.state === 'exited' && c.status.includes('Exited (1)'))) {
						problems.push({ ...c, serverName: env.name });
					}
				}
			}
		}
		loading = false;
	});
</script>

<div class="p-4">
	{#if loading}
		<div class="flex justify-center py-4"><div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else if problems.length === 0}
		<div class="flex items-center gap-2 text-green text-sm">
			<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
			{$t('widget.allHealthy')}
		</div>
	{:else}
		<div class="space-y-2">
			{#each problems.slice(0, 10) as c}
				<div class="flex items-center justify-between gap-2 py-1.5 border-b border-theme last:border-0">
					<div class="flex-1 min-w-0">
						<div class="text-xs font-medium text-primary truncate">{c.name}</div>
						<div class="text-[10px] text-muted">{c.serverName}</div>
					</div>
					<Badge status={c.state} health={extractHealth(c.status)} />
				</div>
			{/each}
			{#if problems.length > 10}
				<div class="text-[10px] text-muted text-center">{$t('widget.more', {count: problems.length - 10})}</div>
			{/if}
		</div>
	{/if}
</div>
