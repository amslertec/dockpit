<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { canDoAction } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';

	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';

	const containerId = $derived($page.params.id);
	const fromStack = $derived($page.url.searchParams.get('stack'));

	$effect(() => {
		if (!$canDoAction('action.container_logs')) goto('/profile');
	});
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
	let viewMode = $state<'logs' | 'analytics'>('logs');

	// Analytics
	interface LogAnalytics {
		totalLines: number;
		errorCount: number;
		warnCount: number;
		infoCount: number;
		errorRate: number;
		topErrors: { message: string; count: number }[];
		timeline: { time: string; errors: number; warns: number }[];
	}

	let analytics = $state<LogAnalytics | null>(null);

	function analyzeLog() {
		const lines = stripAnsi(logs).split('\n').filter(l => l.trim());
		const totalLines = lines.length;
		let errorCount = 0;
		let warnCount = 0;
		let infoCount = 0;
		const errorMessages = new Map<string, number>();
		const timelineBuckets = new Map<string, { errors: number; warns: number }>();

		for (const line of lines) {
			const lower = line.toLowerCase();
			const isError = /\berror\b|\bfatal\b|\bcritical\b|\bpanic\b|\bexception\b|\bfailed\b/.test(lower);
			const isWarn = /\bwarn(ing)?\b|\bdeprecated\b/.test(lower);

			if (isError) {
				errorCount++;
				const match = line.match(/(?:error|fatal|exception|failed)[:\s]*(.{0,80})/i);
				if (match) {
					const msg = match[1].trim().slice(0, 60);
					if (msg) errorMessages.set(msg, (errorMessages.get(msg) || 0) + 1);
				}
			} else if (isWarn) {
				warnCount++;
			} else {
				infoCount++;
			}

			const timeMatch = line.match(/(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2})/);
			if (timeMatch) {
				const bucket = timeMatch[1].slice(0, 16);
				const b = timelineBuckets.get(bucket) || { errors: 0, warns: 0 };
				if (isError) b.errors++;
				if (isWarn) b.warns++;
				timelineBuckets.set(bucket, b);
			}
		}

		const topErrors = [...errorMessages.entries()]
			.sort((a, b) => b[1] - a[1])
			.slice(0, 10)
			.map(([message, count]) => ({ message, count }));

		const timeline = [...timelineBuckets.entries()]
			.sort((a, b) => a[0].localeCompare(b[0]))
			.slice(-30)
			.map(([time, data]) => ({ time: time.slice(11), ...data }));

		analytics = {
			totalLines,
			errorCount,
			warnCount,
			infoCount,
			errorRate: totalLines > 0 ? Math.round((errorCount / totalLines) * 100 * 10) / 10 : 0,
			topErrors,
			timeline,
		};
	}

	function switchToAnalytics() {
		analyzeLog();
		viewMode = 'analytics';
	}

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
			<!-- View mode toggle -->
			<div class="flex items-center bg-[var(--bg-0)] rounded-md border border-theme mr-2">
				<button class="px-2.5 py-1 text-[11px] rounded-md transition {viewMode === 'logs' ? 'bg-accent text-white font-medium' : 'text-secondary hover:text-primary'}" onclick={() => viewMode = 'logs'}>Logs</button>
				<button class="px-2.5 py-1 text-[11px] rounded-md transition {viewMode === 'analytics' ? 'bg-accent text-white font-medium' : 'text-secondary hover:text-primary'}" onclick={() => switchToAnalytics()}>Analytics</button>
			</div>

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

		{#if viewMode === 'logs'}
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
		{:else if analytics}
			<!-- Analytics view -->
			<div class="flex-1 overflow-y-auto p-4 space-y-4">
				<!-- Stats cards -->
				<div class="grid grid-cols-2 md:grid-cols-5 gap-3">
					<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3 text-center">
						<div class="text-2xl font-bold text-primary">{analytics.totalLines}</div>
						<div class="text-[10px] text-muted">Total Lines</div>
					</div>
					<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3 text-center">
						<div class="text-2xl font-bold text-[var(--red)]">{analytics.errorCount}</div>
						<div class="text-[10px] text-muted">Errors</div>
					</div>
					<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3 text-center">
						<div class="text-2xl font-bold text-[var(--yellow)]">{analytics.warnCount}</div>
						<div class="text-[10px] text-muted">Warnings</div>
					</div>
					<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3 text-center">
						<div class="text-2xl font-bold text-[var(--green)]">{analytics.infoCount}</div>
						<div class="text-[10px] text-muted">Info</div>
					</div>
					<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3 text-center">
						<div class="text-2xl font-bold {analytics.errorRate > 10 ? 'text-[var(--red)]' : analytics.errorRate > 5 ? 'text-[var(--yellow)]' : 'text-[var(--green)]'}">{analytics.errorRate}%</div>
						<div class="text-[10px] text-muted">Error Rate</div>
					</div>
				</div>

				<!-- Log Level Distribution Bar -->
				{#if analytics.totalLines > 0}
					<div class="bg-card border border-theme rounded-lg p-4">
						<h4 class="text-xs font-semibold text-primary mb-2">Log Level Distribution</h4>
						<div class="flex h-4 rounded-full overflow-hidden">
							{#if analytics.errorCount > 0}
								<div class="bg-[var(--red)] transition-all" style="width: {(analytics.errorCount / analytics.totalLines * 100)}%" title="{analytics.errorCount} Errors"></div>
							{/if}
							{#if analytics.warnCount > 0}
								<div class="bg-[var(--yellow)] transition-all" style="width: {(analytics.warnCount / analytics.totalLines * 100)}%" title="{analytics.warnCount} Warnings"></div>
							{/if}
							<div class="bg-[var(--green)] transition-all flex-1" title="{analytics.infoCount} Info"></div>
						</div>
						<div class="flex justify-between mt-1.5 text-[9px] text-muted">
							<span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full bg-[var(--red)]"></span> Errors ({analytics.errorCount})</span>
							<span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full bg-[var(--yellow)]"></span> Warnings ({analytics.warnCount})</span>
							<span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full bg-[var(--green)]"></span> Info ({analytics.infoCount})</span>
						</div>
					</div>
				{/if}

				<!-- Error Timeline -->
				{#if analytics.timeline.length > 0}
					<div class="bg-card border border-theme rounded-lg p-4">
						<h4 class="text-xs font-semibold text-primary mb-3">Error Timeline</h4>
						<div class="flex items-end gap-1 h-[120px]">
							{#each analytics.timeline as bucket}
								{@const maxVal = Math.max(...analytics.timeline.map(t => t.errors + t.warns), 1)}
								{@const totalH = ((bucket.errors + bucket.warns) / maxVal) * 100}
								{@const errorH = (bucket.errors / maxVal) * 100}
								<div class="flex-1 flex flex-col justify-end items-center gap-0 group relative">
									<div class="w-full rounded-t" style="height: {errorH}%; min-height: {bucket.errors > 0 ? '2px' : '0'}; background: var(--red);"></div>
									<div class="w-full" style="height: {totalH - errorH}%; min-height: {bucket.warns > 0 ? '2px' : '0'}; background: var(--yellow);"></div>
									<div class="text-[7px] text-muted mt-1 -rotate-45 origin-top-left whitespace-nowrap">{bucket.time}</div>
									<div class="absolute bottom-full mb-1 hidden group-hover:block bg-[var(--dropdown-bg)] border border-[var(--border)] rounded px-2 py-1 text-[9px] text-primary whitespace-nowrap z-10">
										{bucket.time} — {bucket.errors} errors, {bucket.warns} warnings
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Top Errors -->
				{#if analytics.topErrors.length > 0}
					<div class="bg-card border border-theme rounded-lg p-4">
						<h4 class="text-xs font-semibold text-primary mb-3">Top Error Messages</h4>
						<div class="space-y-2">
							{#each analytics.topErrors as err, i}
								<div class="flex items-center gap-3">
									<span class="w-6 h-6 rounded-full bg-[var(--red)]/10 text-[var(--red)] flex items-center justify-center text-[10px] font-bold shrink-0">{i + 1}</span>
									<div class="flex-1 min-w-0">
										<div class="text-xs font-mono text-primary truncate">{err.message}</div>
									</div>
									<span class="px-2 py-0.5 rounded-full bg-[var(--red)]/10 text-[var(--red)] text-[10px] font-bold shrink-0">{err.count}x</span>
								</div>
							{/each}
						</div>
					</div>
				{:else}
					<div class="bg-card border border-theme rounded-lg p-8 text-center">
						<svg class="w-10 h-10 text-[var(--green)] mx-auto mb-2" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
						<p class="text-sm font-medium text-primary">No errors detected</p>
						<p class="text-xs text-muted mt-1">All log entries appear healthy</p>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>
