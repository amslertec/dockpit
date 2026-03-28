<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { favorites } from '$lib/stores/favorites';
	import Badge from '$lib/components/ui/Badge.svelte';
	import { extractHealth } from '$lib/utils/format';
	import type { ContainerInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	let containerMap = $state<Map<string, ContainerInfo>>(new Map());
	let loading = $state(true);

	onMount(async () => {
		// Get unique envIds from favorites
		const envIds = [...new Set($favorites.map(f => f.envId))];
		for (const envId of envIds) {
			const r = await api.get<ContainerInfo[]>(`/env/${envId}/containers`);
			if (r.success && r.data) {
				const m = new Map(containerMap);
				for (const c of r.data) {
					m.set(c.id, c);
				}
				containerMap = m;
			}
		}
		loading = false;
	});
</script>

<div class="p-4">
	{#if loading}
		<div class="flex justify-center py-4"><div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else if $favorites.length === 0}
		<div class="text-center py-4">
			<div class="text-sm text-muted">{$t('favorites.noFavorites')}</div>
			<div class="text-[10px] text-muted mt-1">{$t('favorites.noFavoritesDesc')}</div>
		</div>
	{:else}
		<div class="space-y-2">
			{#each $favorites as fav}
				{@const c = containerMap.get(fav.id)}
				<div class="flex items-center justify-between gap-2 py-1.5 border-b border-theme last:border-0">
					<div class="flex-1 min-w-0">
						<div class="text-xs font-medium text-primary truncate">{fav.name}</div>
						<div class="text-[10px] text-muted truncate">{fav.image}</div>
					</div>
					{#if c}
						<Badge status={c.state} health={extractHealth(c.status)} />
					{:else}
						<span class="text-[10px] text-muted">—</span>
					{/if}
					<div class="flex gap-1">
						<a href="/containers/{fav.id}/logs" class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] text-secondary hover:text-primary transition no-underline" title={$t('containers.logs')}>
							<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg></a>
						<a href="/containers/{fav.id}/terminal" class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] text-secondary hover:text-[var(--green)] transition no-underline" title={$t('containers.terminal')}>
							<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg></a>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
