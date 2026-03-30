<script lang="ts">
	import { onMount } from 'svelte';
	import { selectedEnv } from '$lib/stores/environment';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import Badge from '$lib/components/ui/Badge.svelte';
	import type { ServerOverview, ImageInfo, VolumeInfo, NetworkInfo } from '$lib/api/types';
	import { t } from '$lib/i18n';

	interface Props { server: ServerOverview; }
	let { server: s }: Props = $props();

	const on = $derived(s.info.status === 'online');
	const isLoading = $derived(s.info.status === 'loading');

	let unusedImages = $state<number | null>(null);
	let unusedVolumes = $state<number | null>(null);
	let unusedNetworks = $state<number | null>(null);

	function select() {
		selectedEnv.select(s.id);
		goto('/dashboard');
	}

	onMount(() => {
		if (on) loadUnused();
	});

	// Re-check when status changes to online
	$effect(() => {
		if (on && unusedImages === null) loadUnused();
	});

	async function loadUnused() {
		const [ir, vr, nr] = await Promise.all([
			api.get<ImageInfo[]>(`/env/${s.id}/images`),
			api.get<VolumeInfo[]>(`/env/${s.id}/volumes`),
			api.get<NetworkInfo[]>(`/env/${s.id}/networks`),
		]);
		if (ir.success && ir.data) unusedImages = ir.data.filter(i => !i.in_use).length;
		if (vr.success && vr.data) unusedVolumes = vr.data.filter(v => !v.in_use).length;
		if (nr.success && nr.data) unusedNetworks = nr.data.filter(n => !n.in_use && !['bridge','host','none'].includes(n.name)).length;
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="cursor-pointer" onclick={select}>
	<!-- Header -->
	<div class="flex items-start gap-3 p-4 pb-3">
		<div class="w-9 h-9 rounded-lg flex items-center justify-center shrink-0 {on ? 'bg-green-light' : isLoading ? 'bg-accent-light' : 'bg-red-light'}">
			{#if isLoading}
				<div class="w-4 h-4 border-2 border-[var(--accent)]/30 border-t-[var(--accent)] rounded-full animate-spin"></div>
			{:else}
				<img src="/logo.png" alt="" class="w-5 h-5" />
			{/if}
		</div>
		<div class="flex-1 min-w-0">
			<div class="font-semibold text-primary text-sm">{s.name}</div>
			<div class="mt-0.5"><Badge status={on ? 'online' : isLoading ? 'starting' : 'offline'} /></div>
			<div class="flex flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-[10px] text-secondary">
				<span>{s.is_local ? $t('env.local') : $t('env.agent')}</span>
				{#if on}<span>Docker {s.info.docker_version || '?'}</span>{/if}
				{#if !s.is_local}<span>{s.url.replace('http://', '')}</span>{/if}
			</div>
		</div>
	</div>

	{#if on}
		<!-- Container Stats -->
		<div class="grid grid-cols-4 border-t border-theme bg-1">
			<div class="p-2.5 text-center border-r border-theme">
				<div class="text-base font-bold text-green">{s.info.containers_running}</div>
				<div class="text-[8px] uppercase tracking-wide text-muted mt-0.5">{$t('widget.running')}</div>
			</div>
			<div class="p-2.5 text-center border-r border-theme">
				<div class="text-base font-bold text-red">{s.info.containers_stopped}</div>
				<div class="text-[8px] uppercase tracking-wide text-muted mt-0.5">{$t('dash.stopped')}</div>
			</div>
			<div class="p-2.5 text-center border-r border-theme">
				<div class="text-base font-bold text-primary">{s.info.images}</div>
				<div class="text-[8px] uppercase tracking-wide text-muted mt-0.5">{$t('nav.images')}</div>
			</div>
			<div class="p-2.5 text-center">
				<div class="text-base font-bold text-primary">{s.info.volumes}</div>
				<div class="text-[8px] uppercase tracking-wide text-muted mt-0.5">{$t('nav.volumes')}</div>
			</div>
		</div>

		<!-- System Info -->
		<div class="grid grid-cols-3 border-t border-theme bg-1 px-3 py-2 gap-2">
			<div class="flex items-center gap-1 text-[10px] text-secondary">
				<svg class="w-2.5 h-2.5 opacity-50" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/></svg>
				{s.info.cpus} {$t('widget.cpu')}
			</div>
			<div class="flex items-center gap-1 text-[10px] text-secondary">
				<svg class="w-2.5 h-2.5 opacity-50" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="6" width="20" height="12" rx="2"/></svg>
				{s.info.memory_display}
			</div>
			<div class="flex items-center gap-1 text-[10px] text-secondary">
				<svg class="w-2.5 h-2.5 opacity-50" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="5" r="3"/><circle cx="5" cy="19" r="3"/><circle cx="19" cy="19" r="3"/></svg>
				{s.info.networks} {$t('widget.nets')}
			</div>
		</div>

		<!-- Unused Resources -->
		{#if unusedImages !== null}
			{@const totalUnused = (unusedImages || 0) + (unusedVolumes || 0) + (unusedNetworks || 0)}
			{#if totalUnused > 0}
				<div class="border-t border-theme bg-1 px-3 py-2 flex items-center gap-3">
					<svg class="w-3 h-3 text-yellow shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
					<div class="flex gap-3 text-[9px] text-yellow">
						{#if unusedImages}<span>{unusedImages} {$t('nav.images')}</span>{/if}
						{#if unusedVolumes}<span>{unusedVolumes} {$t('nav.volumes')}</span>{/if}
						{#if unusedNetworks}<span>{unusedNetworks} {$t('widget.nets')}</span>{/if}
						<span class="text-muted">{$t('images.unused').toLowerCase()}</span>
					</div>
				</div>
			{:else}
				<div class="border-t border-theme bg-1 px-3 py-2 flex items-center gap-2">
					<svg class="w-3 h-3 text-green shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
					<span class="text-[9px] text-green">{$t('widget.allClean')}</span>
				</div>
			{/if}
		{/if}
	{:else if isLoading}
		<div class="border-t border-theme px-4 py-3 flex items-center gap-2">
			<div class="w-3 h-3 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div>
			<span class="text-xs text-muted">{$t('widget.connecting')}</span>
		</div>
	{:else}
		<div class="border-t border-theme px-4 py-3 text-muted text-xs">{$t('widget.unreachable')}</div>
	{/if}
</div>
