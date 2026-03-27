<script lang="ts">
	interface Props { status: string; health?: string; }
	let { status, health }: Props = $props();

	const colors: Record<string, string> = {
		running: 'bg-green-light text-green',
		healthy: 'bg-green-light text-green',
		online: 'bg-green-light text-green',
		exited: 'bg-red-light text-red',
		stopped: 'bg-red-light text-red',
		offline: 'bg-red-light text-red',
		dead: 'bg-red-light text-red',
		unhealthy: 'bg-red-light text-red',
		paused: 'bg-yellow-light text-yellow',
		restarting: 'bg-yellow-light text-yellow',
		starting: 'bg-yellow-light text-yellow',
		created: 'bg-accent-light text-accent',
		removing: 'bg-accent-light text-accent',
		partial: 'bg-yellow-light text-yellow',
	};

	// Determine display label: show health if available and running
	const displayLabel = $derived(() => {
		if (health && status === 'running') return health;
		return status;
	});

	const displayColor = $derived(() => {
		if (health && status === 'running') return colors[health] || colors[status] || 'bg-accent-light text-accent';
		return colors[status] || 'bg-accent-light text-accent';
	});
</script>

<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium {displayColor()}">
	<span class="w-1.5 h-1.5 rounded-full bg-current {displayLabel() === 'restarting' || displayLabel() === 'starting' ? 'animate-pulse' : ''}"></span>
	{displayLabel()}
</span>
