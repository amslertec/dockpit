<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';

	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';

	const containerId = $derived($page.params.id);
	const fromStack = $derived($page.url.searchParams.get('stack'));
	const backHref = $derived(fromStack ? `/stacks/${fromStack}` : '/containers');

	let logs = $state('');
	let loading = $state(true);
	let autoRefresh = $state(true);
	let wrapLines = $state(true);
	let tailLines = $state(500);
	let containerName = $state('');
	let logEl: HTMLElement;
	let interval: ReturnType<typeof setInterval> | null = null;
	let autoScroll = $state(true);

	// ANSI color code mapping
	const ansiColors: Record<string, string> = {
		'30': '#555', '31': '#f06060', '32': '#2dd4a0', '33': '#f0b840',
		'34': '#4f8cff', '35': '#a78bfa', '36': '#22d3ee', '37': '#e4e7ee',
		'90': '#888', '91': '#ff7b7b', '92': '#5ef0c0', '93': '#ffe066',
		'94': '#7bb8ff', '95': '#c4a8ff', '96': '#5ef0e0', '97': '#fff',
		'1': 'font-weight:bold', '2': 'opacity:0.7', '3': 'font-style:italic',
		'4': 'text-decoration:underline',
	};

	function parseAnsi(text: string): string {
		// Security: escape ALL HTML first, then only allow safe ANSI color spans
		return text
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;')
			.replace(/"/g, '&quot;')
			.replace(/'/g, '&#39;')
			.replace(/\x1b\[([0-9;]+)m/g, (_, codes: string) => {
				const parts = codes.split(';');
				if (parts.includes('0') || parts.length === 0) return '</span>';
				const styles: string[] = [];
				for (const code of parts) {
					const color = ansiColors[code];
					if (color) {
						// Only allow safe CSS properties (color, font-weight, etc.)
						if (color.startsWith('#') && /^#[0-9a-fA-F]{3,6}$/.test(color)) styles.push(`color:${color}`);
						else if (/^(font-weight|opacity|font-style|text-decoration):[a-z0-9.]+$/.test(color)) styles.push(color);
					}
				}
				return styles.length > 0 ? `<span style="${styles.join(';')}">` : '';
			})
			.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, ''); // Remove remaining escape sequences
	}

	async function fetchLogs() {
		if (!$selectedEnv || !containerId) return;
		const r = await api.get<string>(`/env/${$selectedEnv}/containers/${containerId}/logs?tail=${tailLines}`);
		if (r.success && r.data !== undefined) {
			logs = r.data;
			if (autoScroll && logEl) {
				requestAnimationFrame(() => { logEl.scrollTop = logEl.scrollHeight; });
			}
		}
		loading = false;
	}

	function startAutoRefresh() {
		stopAutoRefresh();
		if (autoRefresh) {
			interval = setInterval(fetchLogs, 3000);
		}
	}

	function stopAutoRefresh() {
		if (interval) { clearInterval(interval); interval = null; }
	}

	function toggleAutoRefresh() {
		autoRefresh = !autoRefresh;
		if (autoRefresh) startAutoRefresh();
		else stopAutoRefresh();
	}

	function handleScroll() {
		if (!logEl) return;
		const threshold = 50;
		autoScroll = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < threshold;
	}

	function stripAnsi(text: string): string {
		return text.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
	}

	function downloadLogs() {
		const clean = stripAnsi(logs);
		const blob = new Blob([clean], { type: 'text/plain' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `${containerName || containerId}_logs.txt`;
		a.click();
		URL.revokeObjectURL(url);
	}

	onMount(async () => {
		// Try to get container name from the containers list
		const r = await api.get<any[]>(`/env/${$selectedEnv}/containers`);
		if (r.success && r.data) {
			const c = r.data.find((x: any) => x.id === containerId || x.id.startsWith(containerId));
			if (c) containerName = c.name;
		}
		await fetchLogs();
		startAutoRefresh();
	});

	onDestroy(() => stopAutoRefresh());

	$effect(() => {
		// Restart refresh when tail changes
		tailLines;
		fetchLogs();
	});
</script>

<svelte:head><title>DockPit — {$t('logs.title', { name: containerName || containerId })}</title></svelte:head>

<div class="flex flex-col h-[calc(100vh-52px-2rem)]">
	<!-- Header -->
	<div class="flex items-center justify-between mb-3 flex-wrap gap-3">
		<div class="flex items-center gap-3">
			<a href={backHref} class="w-8 h-8 flex items-center justify-center rounded-md border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition">
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>
			</a>
			<div>
				<h2 class="text-base font-semibold text-primary">{containerName || 'Container'}</h2>
				<div class="text-[10px] font-mono text-muted">{containerId}</div>
			</div>
		</div>
		<div class="flex items-center gap-2 flex-wrap">
			<!-- Tail lines -->
			<CustomSelect
				options={[
					{ value: 100, label: '100 ' + $t('logs.lines') },
					{ value: 200, label: '200 ' + $t('logs.lines') },
					{ value: 500, label: '500 ' + $t('logs.lines') },
					{ value: 1000, label: '1000 ' + $t('logs.lines') },
					{ value: 5000, label: '5000 ' + $t('logs.lines') }
				]}
				value={tailLines}
				onchange={(v) => tailLines = Number(v)}
				size="sm"
				class="w-[120px]"
			/>

			<!-- Wrap toggle -->
			<button onclick={() => wrapLines = !wrapLines}
				class="inline-flex items-center gap-1.5 px-2.5 py-1.5 border rounded-md text-xs transition
				{wrapLines ? 'border-[var(--accent)] text-accent bg-accent-light' : 'border-theme text-secondary hover:text-primary'}">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M3 12h15a3 3 0 110 6H9m0 0l3-3m-3 3l3 3"/></svg>
				{$t('logs.wrap')}
			</button>

			<!-- Auto-refresh toggle -->
			<button onclick={toggleAutoRefresh}
				class="inline-flex items-center gap-1.5 px-2.5 py-1.5 border rounded-md text-xs transition
				{autoRefresh ? 'border-[var(--green)] text-green bg-green-light' : 'border-theme text-secondary hover:text-primary'}">
				<span class="w-2 h-2 rounded-full {autoRefresh ? 'bg-[var(--green)] animate-pulse' : 'bg-[var(--text-muted)]'}"></span>
				{$t('logs.live')}
			</button>

			<!-- Download -->
			<button onclick={downloadLogs}
				class="inline-flex items-center gap-1.5 px-2.5 py-1.5 border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 rounded-md text-xs transition">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
				{$t('logs.download')}
			</button>

			<!-- Manual refresh -->
			<button onclick={fetchLogs}
				class="inline-flex items-center justify-center w-8 h-8 border border-theme text-[var(--green)] hover:border-[var(--green)]/40 hover:bg-[var(--green)]/8 rounded-md transition">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</button>
		</div>
	</div>

	<!-- Log output -->
	<div class="bg-card border border-theme rounded-lg flex-1 flex flex-col overflow-hidden">
		<div class="px-4 py-2 border-b border-theme flex items-center justify-between">
			<span class="text-xs text-secondary">
				{logs.split('\n').filter(l => l.trim()).length} {$t('logs.lines')}
				{#if autoRefresh}
					<span class="text-green ml-2">· {$t('logs.live')}</span>
				{/if}
			</span>
			{#if autoScroll}
				<span class="text-[10px] text-muted">{$t('logs.autoScroll')}</span>
			{/if}
		</div>

		{#if loading}
			<div class="flex-1 flex items-center justify-center">
				<div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div>
			</div>
		{:else}
			<div bind:this={logEl} onscroll={handleScroll}
				class="flex-1 overflow-auto p-4 font-mono text-[12px] leading-[1.7] bg-0
				{wrapLines ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'}">
				{@html parseAnsi(logs) || `<span class="text-muted">${$t('logs.noLogs')}</span>`}
			</div>
		{/if}
	</div>
</div>
