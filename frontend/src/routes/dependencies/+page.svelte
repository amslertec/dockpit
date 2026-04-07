<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { canSeePage } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import type { ContainerInfo, NetworkInfo, VolumeInfo } from '$lib/api/types';

	$effect(() => {
		if (!$canSeePage('page.containers')) goto('/profile');
	});

	let containers = $state<ContainerInfo[]>([]);
	let networks = $state<NetworkInfo[]>([]);
	let volumes = $state<VolumeInfo[]>([]);
	let loading = $state(true);
	let svgEl: SVGSVGElement;
	let tooltip = $state<{ x: number; y: number; text: string } | null>(null);

	// Layout
	let width = $state(1200);
	let height = $state(800);

	interface Node {
		id: string;
		label: string;
		type: 'container' | 'network' | 'volume';
		x: number;
		y: number;
		color: string;
		state?: string;
		stack?: string;
	}

	interface Edge {
		from: string;
		to: string;
		type: 'network' | 'volume';
		label?: string;
	}

	let nodes = $state<Node[]>([]);
	let edges = $state<Edge[]>([]);

	onMount(() => load());
	$effect(() => { $selectedEnv; load(); });

	// Container inspect data: maps container ID → { networks, mounts }
	let inspectData = $state<Map<string, { networks: string[]; volumes: string[] }>>(new Map());

	async function load() {
		if (!$selectedEnv) return;
		loading = true;
		const [cR, nR, vR] = await Promise.all([
			api.get<ContainerInfo[]>(`/env/${$selectedEnv}/containers`),
			api.get<NetworkInfo[]>(`/env/${$selectedEnv}/networks`),
			api.get<VolumeInfo[]>(`/env/${$selectedEnv}/volumes`),
		]);
		if (cR.success && cR.data) containers = cR.data;
		if (nR.success && nR.data) networks = nR.data;
		if (vR.success && vR.data) volumes = vR.data;

		// Fetch inspect for each container to get network/volume mappings
		const newInspect = new Map<string, { networks: string[]; volumes: string[] }>();
		await Promise.all(containers.map(async (c) => {
			const r = await api.get<any>(`/env/${$selectedEnv}/containers/${c.id}/inspect`);
			if (r.success && r.data) {
				const nets = Object.keys(r.data.NetworkSettings?.Networks || {});
				const vols = (r.data.Mounts || [])
					.filter((m: any) => m.Type === 'volume')
					.map((m: any) => m.Name || m.Source);
				newInspect.set(c.id, { networks: nets, volumes: vols });
			}
		}));
		inspectData = newInspect;

		loading = false;
		buildGraph();
	}

	function buildGraph() {
		const newNodes: Node[] = [];
		const newEdges: Edge[] = [];

		// Group containers by stack
		const stacks = new Map<string, ContainerInfo[]>();
		const standalone: ContainerInfo[] = [];
		for (const c of containers) {
			if (c.stack_name) {
				if (!stacks.has(c.stack_name)) stacks.set(c.stack_name, []);
				stacks.get(c.stack_name)!.push(c);
			} else {
				standalone.push(c);
			}
		}

		// Layout: containers in center, networks on left, volumes on right
		const totalContainers = containers.length;
		const containerSpacing = Math.min(100, Math.max(60, 700 / Math.max(totalContainers, 1)));
		let cy = 80;

		// Add stack groups
		for (const [stackName, stackContainers] of stacks) {
			for (let i = 0; i < stackContainers.length; i++) {
				const c = stackContainers[i];
				newNodes.push({
					id: c.id, label: c.name, type: 'container',
					x: width / 2, y: cy,
					color: c.state === 'running' ? 'var(--green)' : c.state === 'exited' ? 'var(--red)' : 'var(--yellow)',
					state: c.state, stack: stackName,
				});
				cy += containerSpacing;
			}
			cy += 20;
		}

		// Add standalone containers
		for (const c of standalone) {
			newNodes.push({
				id: c.id, label: c.name, type: 'container',
				x: width / 2, y: cy,
				color: c.state === 'running' ? 'var(--green)' : c.state === 'exited' ? 'var(--red)' : 'var(--yellow)',
				state: c.state,
			});
			cy += containerSpacing;
		}

		height = Math.max(600, cy + 100);

		// Add networks (left side)
		const usedNetworks = networks.filter(n => n.containers_count > 0 && !['bridge', 'host', 'none'].includes(n.name));
		const netSpacing = Math.max(60, (height - 100) / Math.max(usedNetworks.length, 1));
		for (let i = 0; i < usedNetworks.length; i++) {
			const n = usedNetworks[i];
			newNodes.push({
				id: `net-${n.id}`, label: n.name, type: 'network',
				x: 120, y: 80 + i * netSpacing,
				color: 'var(--accent)',
			});
		}

		// Add volumes (right side) - only in-use
		const usedVolumes = volumes.filter(v => v.in_use);
		const volSpacing = Math.max(60, (height - 100) / Math.max(usedVolumes.length, 1));
		for (let i = 0; i < usedVolumes.length; i++) {
			const v = usedVolumes[i];
			newNodes.push({
				id: `vol-${v.name}`, label: v.name.length > 20 ? v.name.slice(0, 20) + '...' : v.name, type: 'volume',
				x: width - 120, y: 80 + i * volSpacing,
				color: 'var(--purple)',
			});
		}

		// Build edges from real inspect data (networks + volumes)
		for (const c of containers) {
			const info = inspectData.get(c.id);
			if (!info) continue;

			// Network edges
			for (const netName of info.networks) {
				const netNode = newNodes.find(n => n.type === 'network' && n.label === netName);
				if (netNode) {
					newEdges.push({ from: c.id, to: netNode.id, type: 'network' });
				}
			}

			// Volume edges
			for (const volName of info.volumes) {
				const volNode = newNodes.find(n => n.type === 'volume' && (n.id === `vol-${volName}` || n.label === volName || n.label === (volName.length > 20 ? volName.slice(0, 20) + '...' : volName)));
				if (volNode) {
					newEdges.push({ from: c.id, to: volNode.id, type: 'volume' });
				}
			}
		}

		// Connect containers in same stack to each other (implicit dependency)
		for (const [_, stackContainers] of stacks) {
			for (let i = 0; i < stackContainers.length - 1; i++) {
				newEdges.push({ from: stackContainers[i].id, to: stackContainers[i + 1].id, type: 'network', label: 'stack' });
			}
		}

		nodes = newNodes;
		edges = newEdges;
	}

	function showTooltip(e: MouseEvent, node: Node) {
		tooltip = {
			x: e.clientX + 10,
			y: e.clientY + 10,
			text: `${node.label}\n${node.type === 'container' ? (node.state || '') + (node.stack ? ' · Stack: ' + node.stack : '') : node.type}`,
		};
	}

	function hideTooltip() { tooltip = null; }

	function nodeIcon(type: string): string {
		if (type === 'container') return '\u{1F4E6}';
		if (type === 'network') return '\u{1F310}';
		if (type === 'volume') return '\u{1F4BE}';
		return '?';
	}
</script>

<svelte:head><title>DockPit — {$t('nav.dependencies')}</title></svelte:head>

<div class="space-y-4">
	<div class="flex items-center justify-between flex-wrap gap-3">
		<div>
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('nav.dependencies')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{containers.length} Containers · {networks.filter(n => n.containers_count > 0).length} Networks · {volumes.filter(v => v.in_use).length} Volumes</p>
		</div>
		<Button variant="success" size="sm" onclick={load} title={$t('common.refresh')}>
			<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
		</Button>
	</div>

	<div class="bg-card border border-theme rounded-lg overflow-hidden">
		{#if loading}
			<div class="flex justify-center py-16"><div class="w-6 h-6 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
		{:else if containers.length === 0}
			<div class="text-center py-16 text-sm text-muted">{$t('common.noResults')}</div>
		{:else}
			<!-- Legend -->
			<div class="px-4 py-2.5 border-b border-theme bg-[var(--bg-1)] flex items-center gap-4 text-[10px] text-muted">
				<span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-full bg-[var(--green)]"></span> Running</span>
				<span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-full bg-[var(--red)]"></span> Stopped</span>
				<span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded bg-[var(--accent)]/20 border border-[var(--accent)]"></span> Network</span>
				<span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded bg-[var(--purple)]/20 border border-[var(--purple)]"></span> Volume</span>
				<span class="flex items-center gap-1.5"><span class="w-6 h-px bg-[var(--accent)]"></span> Connection</span>
			</div>

			<div class="overflow-auto" style="max-height: calc(100vh - 220px);">
				<svg bind:this={svgEl} {width} {height} class="w-full" style="min-width: {width}px; min-height: {height}px;">
					<!-- Background grid -->
					<defs>
						<pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
							<path d="M 40 0 L 0 0 0 40" fill="none" stroke="var(--border)" stroke-width="0.5" opacity="0.3"/>
						</pattern>
					</defs>
					<rect {width} {height} fill="url(#grid)" rx="0"/>

					<!-- Edges -->
					{#each edges as edge}
						{@const fromNode = nodes.find(n => n.id === edge.from)}
						{@const toNode = nodes.find(n => n.id === edge.to)}
						{#if fromNode && toNode}
							<line
								x1={fromNode.x} y1={fromNode.y}
								x2={toNode.x} y2={toNode.y}
								stroke={edge.type === 'network' ? 'var(--accent)' : 'var(--purple)'}
								stroke-width="1.5"
								stroke-dasharray={edge.label === 'stack' ? '4,4' : 'none'}
								opacity="0.4"
							/>
						{/if}
					{/each}

					<!-- Stack backgrounds -->
					{#each [...new Set(nodes.filter(n => n.stack).map(n => n.stack))] as stackName}
						{@const stackNodes = nodes.filter(n => n.stack === stackName)}
						{#if stackNodes.length > 0}
							{@const minY = Math.min(...stackNodes.map(n => n.y)) - 30}
							{@const maxY = Math.max(...stackNodes.map(n => n.y)) + 30}
							<rect
								x={width / 2 - 120} y={minY}
								width="240" height={maxY - minY}
								rx="12" fill="var(--accent)" opacity="0.05"
								stroke="var(--accent)" stroke-width="1" stroke-dasharray="4,4"
							/>
							<text x={width / 2} y={minY + 14} text-anchor="middle" fill="var(--accent)" font-size="10" font-weight="600" opacity="0.6">
								{stackName}
							</text>
						{/if}
					{/each}

					<!-- Nodes -->
					{#each nodes as node}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<g
							class="cursor-pointer"
							transform="translate({node.x}, {node.y})"
							onmouseenter={(e) => showTooltip(e, node)}
							onmouseleave={hideTooltip}
							onclick={() => { if (node.type === 'container') goto(`/containers/${node.id}`); }}
						>
							{#if node.type === 'container'}
								<rect x="-80" y="-18" width="160" height="36" rx="8"
									fill="var(--bg-card)" stroke={node.color} stroke-width="2"/>
								<circle cx="-64" cy="0" r="4" fill={node.color}/>
								<text x="-52" y="4" fill="var(--text)" font-size="11" font-weight="500">
									{node.label.length > 18 ? node.label.slice(0, 18) + '\u2026' : node.label}
								</text>
							{:else if node.type === 'network'}
								<rect x="-60" y="-14" width="120" height="28" rx="14"
									fill="var(--accent)" fill-opacity="0.1" stroke="var(--accent)" stroke-width="1.5"/>
								<text x="0" y="4" text-anchor="middle" fill="var(--accent)" font-size="10" font-weight="600">
									{node.label.length > 14 ? node.label.slice(0, 14) + '\u2026' : node.label}
								</text>
							{:else}
								<rect x="-60" y="-14" width="120" height="28" rx="6"
									fill="var(--purple)" fill-opacity="0.1" stroke="var(--purple)" stroke-width="1.5"/>
								<text x="0" y="4" text-anchor="middle" fill="var(--purple)" font-size="10" font-weight="600">
									{node.label}
								</text>
							{/if}
						</g>
					{/each}
				</svg>
			</div>
		{/if}
	</div>
</div>

<!-- Tooltip -->
{#if tooltip}
	<div class="fixed z-[9999] bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-lg shadow-lg px-3 py-2 text-xs text-primary whitespace-pre-line pointer-events-none"
		style="left: {tooltip.x}px; top: {tooltip.y}px;">
		{tooltip.text}
	</div>
{/if}
