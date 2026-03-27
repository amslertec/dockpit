<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import type { ImageInfo, VolumeInfo, NetworkInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	let unusedImages = $state(0);
	let unusedVolumes = $state(0);
	let unusedNetworks = $state(0);
	let loading = $state(true);

	onMount(async () => {
		for (const env of $environments) {
			const [ir, vr, nr] = await Promise.all([
				api.get<ImageInfo[]>(`/env/${env.id}/images`),
				api.get<VolumeInfo[]>(`/env/${env.id}/volumes`),
				api.get<NetworkInfo[]>(`/env/${env.id}/networks`),
			]);
			if (ir.success && ir.data) unusedImages += ir.data.filter(i => !i.in_use).length;
			if (vr.success && vr.data) unusedVolumes += vr.data.filter(v => !v.in_use).length;
			if (nr.success && nr.data) unusedNetworks += nr.data.filter(n => !n.in_use && !['bridge','host','none'].includes(n.name)).length;
		}
		loading = false;
	});

	const total = $derived(unusedImages + unusedVolumes + unusedNetworks);
</script>

<div class="p-4">
	{#if loading}
		<div class="flex justify-center py-4"><div class="w-4 h-4 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else if total === 0}
		<div class="flex items-center gap-2 text-green text-sm">
			<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
			{$t('widget.allClean')}
		</div>
	{:else}
		<div class="space-y-3">
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-2">
					<div class="w-8 h-8 rounded-lg bg-purple-light flex items-center justify-center"><span class="text-sm font-bold text-purple">{unusedImages}</span></div>
					<div><div class="text-xs text-primary">{$t('widget.unusedImages')}</div><div class="text-[9px] text-muted">{$t('widget.canBeDeleted')}</div></div>
				</div>
				{#if unusedImages > 0}<a href="/images" class="text-[10px] text-accent hover:text-accent-hover">{$t('widget.cleanup')}</a>{/if}
			</div>
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-2">
					<div class="w-8 h-8 rounded-lg bg-yellow-light flex items-center justify-center"><span class="text-sm font-bold text-yellow">{unusedVolumes}</span></div>
					<div><div class="text-xs text-primary">{$t('widget.unusedVolumes')}</div><div class="text-[9px] text-muted">{$t('widget.takeStorage')}</div></div>
				</div>
				{#if unusedVolumes > 0}<a href="/volumes" class="text-[10px] text-accent hover:text-accent-hover">{$t('widget.cleanup')}</a>{/if}
			</div>
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-2">
					<div class="w-8 h-8 rounded-lg bg-accent-light flex items-center justify-center"><span class="text-sm font-bold text-accent">{unusedNetworks}</span></div>
					<div><div class="text-xs text-primary">{$t('widget.unusedNetworks')}</div><div class="text-[9px] text-muted">{$t('widget.orphanedNetworks')}</div></div>
				</div>
				{#if unusedNetworks > 0}<a href="/networks" class="text-[10px] text-accent hover:text-accent-hover">{$t('widget.cleanup')}</a>{/if}
			</div>
		</div>
	{/if}
</div>
