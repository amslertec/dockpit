<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import Badge from '$lib/components/ui/Badge.svelte';
	import type { StackInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	let stacks = $state<(StackInfo & { serverName: string })[]>([]);
	let loading = $state(true);

	onMount(async () => {
		for (const env of $environments) {
			const r = await api.get<StackInfo[]>(`/env/${env.id}/stacks`);
			if (r.success && r.data) {
				for (const s of r.data) {
					stacks.push({ ...s, serverName: env.name });
				}
			}
		}
		stacks.sort((a, b) => { const o: Record<string, number> = { running: 0, partial: 1, stopped: 2 }; return (o[a.status] ?? 3) - (o[b.status] ?? 3); });
		loading = false;
	});

	const running = $derived(stacks.filter(s => s.status === 'running').length);
	const partial = $derived(stacks.filter(s => s.status === 'partial').length);
	const stopped = $derived(stacks.filter(s => s.status === 'stopped').length);
</script>

<div class="p-4">
	{#if loading}
		<div class="flex justify-center py-4"><div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<!-- Summary -->
		<div class="flex gap-4 mb-3 text-center">
			<div><div class="text-lg font-bold text-green">{running}</div><div class="text-[9px] text-muted">{$t('stacks.active')}</div></div>
			<div><div class="text-lg font-bold text-yellow">{partial}</div><div class="text-[9px] text-muted">{$t('stacks.partial')}</div></div>
			<div><div class="text-lg font-bold text-red">{stopped}</div><div class="text-[9px] text-muted">{$t('stacks.stopped')}</div></div>
		</div>
		<!-- List -->
		<div class="space-y-1.5">
			{#each stacks.slice(0, 12) as s}
				<div class="flex items-center justify-between gap-2">
					<a href="/stacks/{s.name}" class="text-xs text-primary hover:text-accent transition truncate flex-1">{s.name}</a>
					<div class="flex items-center gap-2 shrink-0">
						<span class="text-[10px] text-muted">{s.running_services}/{s.services_count}</span>
						<Badge status={s.status} />
					</div>
				</div>
			{/each}
			{#if stacks.length > 12}
				<a href="/stacks" class="block text-[10px] text-accent text-center mt-1">{$t('widget.allStacks', {count: stacks.length})}</a>
			{/if}
		</div>
	{/if}
</div>
