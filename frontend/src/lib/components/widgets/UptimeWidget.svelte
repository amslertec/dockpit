<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import type { ContainerInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	let containers = $state<{ name: string; server: string; uptime: string; created: number }[]>([]);
	let loading = $state(true);

	function formatUptime(status: string): string {
		const m = status.match(/Up\s+(.+?)(\s+\(|$)/);
		return m ? m[1] : status;
	}

	onMount(async () => {
		for (const env of $environments) {
			const r = await api.get<ContainerInfo[]>(`/env/${env.id}/containers`);
			if (r.success && r.data) {
				for (const c of r.data) {
					if (c.state === 'running') {
						containers.push({ name: c.name, server: env.name, uptime: formatUptime(c.status), created: c.created });
					}
				}
			}
		}
		containers.sort((a, b) => a.created - b.created); // Longest running first
		loading = false;
	});
</script>

<div class="p-4">
	{#if loading}
		<div class="flex justify-center py-4"><div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="space-y-1.5">
			{#each containers.slice(0, 10) as c}
				<div class="flex items-center justify-between gap-2">
					<div class="flex-1 min-w-0">
						<div class="text-xs text-primary truncate">{c.name}</div>
						<div class="text-[9px] text-muted">{c.server}</div>
					</div>
					<span class="text-[11px] text-green font-mono shrink-0">{c.uptime}</span>
				</div>
			{/each}
			{#if containers.length > 10}
				<div class="text-[10px] text-muted text-center">{$t('widget.more', {count: containers.length - 10})}</div>
			{/if}
			{#if containers.length === 0}
				<div class="text-xs text-muted text-center py-2">{$t('widget.noRunning')}</div>
			{/if}
		</div>
	{/if}
</div>
