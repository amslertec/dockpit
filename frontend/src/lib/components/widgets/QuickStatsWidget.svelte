<script lang="ts">
	import type { ServerOverview } from '$lib/api/types';
	import { t } from '$lib/i18n';

	interface Props { servers: ServerOverview[]; }
	let { servers }: Props = $props();

	const online = $derived(servers.filter(s => s.info.status === 'online'));
	const totalRunning = $derived(online.reduce((a, s) => a + s.info.containers_running, 0));
	const totalStopped = $derived(online.reduce((a, s) => a + s.info.containers_stopped, 0));
	const totalImages = $derived(online.reduce((a, s) => a + s.info.images, 0));
	const totalVolumes = $derived(online.reduce((a, s) => a + s.info.volumes, 0));
	const totalNetworks = $derived(online.reduce((a, s) => a + s.info.networks, 0));
	const totalContainers = $derived(totalRunning + totalStopped);
</script>

<div class="p-4">
	<div class="grid grid-cols-3 md:grid-cols-6 gap-3">
		<div class="text-center">
			<div class="text-2xl font-bold text-accent">{totalContainers}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('nav.containers')}</div>
		</div>
		<div class="text-center">
			<div class="text-2xl font-bold text-green">{totalRunning}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('dash.active')}</div>
		</div>
		<div class="text-center">
			<div class="text-2xl font-bold text-red">{totalStopped}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('dash.stopped')}</div>
		</div>
		<div class="text-center">
			<div class="text-2xl font-bold text-purple">{totalImages}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('nav.images')}</div>
		</div>
		<div class="text-center">
			<div class="text-2xl font-bold text-yellow">{totalVolumes}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('nav.volumes')}</div>
		</div>
		<div class="text-center">
			<div class="text-2xl font-bold text-accent">{totalNetworks}</div>
			<div class="text-[10px] text-muted mt-0.5">{$t('nav.networks')}</div>
		</div>
	</div>

	<!-- Container bar -->
	{#if totalContainers > 0}
		<div class="mt-4">
			<div class="flex items-center justify-between text-[10px] text-secondary mb-1.5">
				<span>{totalRunning} {$t('dash.active').toLowerCase()}</span>
				<span>{totalStopped} {$t('dash.stopped').toLowerCase()}</span>
			</div>
			<div class="flex h-1.5 rounded-full overflow-hidden bg-0">
				<div class="bg-[var(--green)] transition-all" style="width:{Math.round(totalRunning / totalContainers * 100)}%"></div>
				<div class="bg-[var(--red)] transition-all" style="width:{Math.round(totalStopped / totalContainers * 100)}%"></div>
			</div>
		</div>
	{/if}

	<div class="mt-3 text-[10px] text-muted">{$t('widget.serverOnline', {count: online.length, total: servers.length})}</div>
</div>
