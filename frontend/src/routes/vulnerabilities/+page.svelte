<script lang="ts">
	import { onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { canSeePage, canDoAction } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import { toasts } from '$lib/stores/toast';
	import Button from '$lib/components/ui/Button.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import { formatDateTime } from '$lib/utils/format';
	import type { VulnerabilityScan, VulnScanStatus } from '$lib/api/types';

	$effect(() => {
		if (!$canSeePage('page.vulnerabilities')) goto('/profile');
	});

	let scans = $state<VulnerabilityScan[]>([]);
	let loading = $state(true);
	let scanning = $state(false);
	let scanTotal = $state(0);
	let scanDone = $state(0);
	let scanningImage = $state<string | null>(null);
	let pollInterval: ReturnType<typeof setInterval> | null = null;
	let expandedRows = $state<Set<number>>(new Set());
	let cveShowAll = $state<Set<number>>(new Set());
	const CVE_PAGE_SIZE = 20;
	let page = $state(1);
	let perPage = $state(25);

	const totalCritical = $derived(scans.reduce((s, r) => s + r.critical, 0));
	const totalHigh = $derived(scans.reduce((s, r) => s + r.high, 0));
	const totalMedium = $derived(scans.reduce((s, r) => s + r.medium, 0));
	const totalLow = $derived(scans.reduce((s, r) => s + r.low, 0));

	const paged = $derived(
		perPage === 0 ? scans : scans.slice((page - 1) * perPage, page * perPage)
	);

	onDestroy(() => { if (pollInterval) clearInterval(pollInterval); });
	$effect(() => { $selectedEnv; loadScans(); checkStatus(); });

	async function checkStatus() {
		if (!$selectedEnv) return;
		const r = await api.get<VulnScanStatus>(`/env/${$selectedEnv}/vulnerabilities/status`);
		if (r.success && r.data?.running) {
			scanning = true;
			scanTotal = r.data.total;
			scanDone = r.data.done;
			startPolling();
		}
	}

	function startPolling() {
		if (pollInterval) clearInterval(pollInterval);
		pollInterval = setInterval(pollStatus, 3000);
	}
	function stopPolling() {
		if (pollInterval) { clearInterval(pollInterval); pollInterval = null; }
	}

	async function loadScans() {
		if (!$selectedEnv) return;
		loading = true;
		const r = await api.get<VulnerabilityScan[]>(`/env/${$selectedEnv}/vulnerabilities`);
		if (r.success && r.data) {
			scans = r.data;
		}
		loading = false;
	}

	async function pollStatus() {
		if (!$selectedEnv) return;
		const r = await api.get<VulnScanStatus>(`/env/${$selectedEnv}/vulnerabilities/status`);
		if (r.success && r.data) {
			const prevDone = scanDone;
			scanTotal = r.data.total;
			scanDone = r.data.done;
			if (!r.data.running) {
				stopPolling();
				scanning = false;
				await loadScans();
				toasts.success($t('vuln.scanComplete'));
			} else if (r.data.done > prevDone) {
				// Only refresh full list when new results arrived
				const sr = await api.get<VulnerabilityScan[]>(`/env/${$selectedEnv}/vulnerabilities`);
				if (sr.success && sr.data) scans = sr.data;
			}
		}
	}

	async function scanAll() {
		if (!$selectedEnv || scanning) return;
		scanning = true;
		scanTotal = 0;
		scanDone = 0;
		const r = await api.post(`/env/${$selectedEnv}/vulnerabilities/scan`, {});
		if (r.success) {
			toasts.success($t('vuln.scanStarted'));
			startPolling();
		} else {
			toasts.error(r.error || $t('common.error'));
			scanning = false;
		}
	}

	async function scanImage(image: string) {
		if (!$selectedEnv) return;
		scanningImage = image;
		const encoded = encodeURIComponent(image);
		const r = await api.post(`/env/${$selectedEnv}/vulnerabilities/scan/${encoded}`, {});
		if (r.success) {
			startPolling();
		} else {
			toasts.error(r.error || $t('common.error'));
		}
		scanningImage = null;
	}

	function toggleRow(id: number | undefined) {
		if (id == null) return;
		const next = new Set(expandedRows);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expandedRows = next;
	}

	function parseCves(scan: VulnerabilityScan): Array<{id: string; severity: string; package: string; version: string; fixed: string; description: string}> {
		try {
			return JSON.parse(scan.cves_json || '[]');
		} catch {
			return [];
		}
	}

	function severityBadgeClass(severity: string): string {
		switch (severity.toLowerCase()) {
			case 'critical': return 'bg-[var(--red-bg)] text-[var(--red)]';
			case 'high': return 'bg-[#fff3e0] text-[#e65100] dark:bg-[#e6510020] dark:text-[#ff9800]';
			case 'medium': return 'bg-[var(--yellow-bg)] text-[var(--yellow)]';
			case 'low': return 'bg-[var(--bg-hover)] text-[var(--text-muted)]';
			default: return 'bg-[var(--bg-hover)] text-[var(--text-secondary)]';
		}
	}

	function handlePageChange(p: number, pp: number) {
		page = p;
		perPage = pp;
	}
</script>

<div class="space-y-4">
	<!-- Header -->
	<div class="flex items-center justify-between flex-wrap gap-3">
		<div>
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('vuln.title')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{scans.length} {$t('vuln.imagesScanned').toLowerCase()}</p>
		</div>
		<div class="flex items-center gap-2">
			<Button size="sm" variant="success" onclick={loadScans} disabled={loading} title={$t('common.refresh')}>
				<svg class="w-3.5 h-3.5 {loading ? 'animate-spin' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6"/><path d="M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</Button>
			{#if $canDoAction('action.vuln_scan')}
			<Button size="sm" variant="primary" onclick={scanAll} disabled={scanning} title={$t('vuln.scanAll')}>
				{#if scanning}
					<div class="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
				{:else}
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
				{/if}
			</Button>
			{/if}
		</div>
	</div>

	<!-- Scanning Banner -->
	{#if scanning}
		<div class="flex items-center gap-3 px-4 py-3 rounded-[var(--radius-lg)] border border-[var(--accent)]/30 bg-[var(--accent-bg)] mb-3">
			<div class="w-4 h-4 border-2 border-[var(--accent)]/30 border-t-[var(--accent)] rounded-full animate-spin shrink-0"></div>
			<span class="text-xs text-[var(--accent)] font-medium">
				{$t('vuln.scanRunning')}
				{#if scanTotal > 0}
					— {scanDone}/{scanTotal} images
				{/if}
			</span>
		</div>
	{/if}

	<!-- Info Banner -->
	<div class="flex items-start gap-3 px-4 py-3 rounded-[var(--radius-lg)] border border-[var(--accent)]/30 bg-[var(--accent-bg)]">
		<svg class="w-5 h-5 text-[var(--accent)] shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
		<p class="text-xs text-[var(--text-secondary)] leading-relaxed">{$t('vuln.loginRequired')}</p>
	</div>

	<!-- Summary Cards -->
	<div class="grid grid-cols-2 md:grid-cols-5 gap-3">
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<p class="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.imagesScanned')}</p>
			<p class="text-2xl font-bold text-[var(--text)] mt-1">{scans.length}</p>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<p class="text-[10px] font-semibold uppercase tracking-wider text-[var(--red)]">{$t('vuln.critical')}</p>
			<p class="text-2xl font-bold text-[var(--red)] mt-1">{totalCritical}</p>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<p class="text-[10px] font-semibold uppercase tracking-wider text-[#e65100] dark:text-[#ff9800]">{$t('vuln.high')}</p>
			<p class="text-2xl font-bold text-[#e65100] dark:text-[#ff9800] mt-1">{totalHigh}</p>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<p class="text-[10px] font-semibold uppercase tracking-wider text-[var(--yellow)]">{$t('vuln.medium')}</p>
			<p class="text-2xl font-bold text-[var(--yellow)] mt-1">{totalMedium}</p>
		</div>
		<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] p-4">
			<p class="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.low')}</p>
			<p class="text-2xl font-bold text-[var(--text-muted)] mt-1">{totalLow}</p>
		</div>
	</div>

	<!-- Table -->
	<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] overflow-hidden">
		{#if loading}
			<div class="flex items-center justify-center py-16">
				<div class="w-6 h-6 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
			</div>
		{:else if scans.length === 0}
			<div class="flex flex-col items-center justify-center py-16 text-center">
				<svg class="w-10 h-10 text-[var(--text-muted)] mb-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
				<p class="text-sm font-medium text-[var(--text-secondary)]">{$t('vuln.noScans')}</p>
				<p class="text-xs text-[var(--text-muted)] mt-1 max-w-[300px]">{$t('vuln.noScansDesc')}</p>
			</div>
		{:else}
			<div class="overflow-x-auto">
				<table class="w-full text-sm">
					<thead>
						<tr class="border-b border-[var(--border)]">
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.image')}</th>
							<th class="text-center px-3 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--red)]">{$t('vuln.critical')}</th>
							<th class="text-center px-3 py-3 text-[11px] font-semibold uppercase tracking-wider text-[#e65100] dark:text-[#ff9800]">{$t('vuln.high')}</th>
							<th class="text-center px-3 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--yellow)]">{$t('vuln.medium')}</th>
							<th class="text-center px-3 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.low')}</th>
							<th class="text-center px-3 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.total')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.lastScan')}</th>
							<th class="text-right px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('common.actions')}</th>
						</tr>
					</thead>
					<tbody>
						{#each paged as scan (scan.id ?? scan.image)}
							<!-- svelte-ignore a11y_click_events_have_key_events -->
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<tr
								class="border-b border-[var(--border)] last:border-0 hover:bg-[var(--bg-hover)] transition-colors duration-150 cursor-pointer"
								onclick={() => toggleRow(scan.id)}
							>
								<td class="px-4 py-3 text-xs text-[var(--text)] font-medium max-w-[250px] truncate" title={scan.image}>{scan.image}</td>
								<td class="px-3 py-3 text-center">
									{#if scan.critical > 0}
										<span class="inline-flex items-center justify-center min-w-[24px] px-1.5 py-0.5 rounded-full text-[11px] font-bold bg-[var(--red-bg)] text-[var(--red)]">{scan.critical}</span>
									{:else}
										<span class="text-xs text-[var(--text-muted)]">0</span>
									{/if}
								</td>
								<td class="px-3 py-3 text-center">
									{#if scan.high > 0}
										<span class="inline-flex items-center justify-center min-w-[24px] px-1.5 py-0.5 rounded-full text-[11px] font-bold bg-[#fff3e0] text-[#e65100] dark:bg-[#e6510020] dark:text-[#ff9800]">{scan.high}</span>
									{:else}
										<span class="text-xs text-[var(--text-muted)]">0</span>
									{/if}
								</td>
								<td class="px-3 py-3 text-center">
									{#if scan.medium > 0}
										<span class="inline-flex items-center justify-center min-w-[24px] px-1.5 py-0.5 rounded-full text-[11px] font-bold bg-[var(--yellow-bg)] text-[var(--yellow)]">{scan.medium}</span>
									{:else}
										<span class="text-xs text-[var(--text-muted)]">0</span>
									{/if}
								</td>
								<td class="px-3 py-3 text-center">
									<span class="text-xs text-[var(--text-muted)]">{scan.low}</span>
								</td>
								<td class="px-3 py-3 text-center">
									<span class="text-xs font-medium text-[var(--text-secondary)]">{scan.total}</span>
								</td>
								<td class="px-4 py-3 text-xs text-[var(--text-muted)] whitespace-nowrap">{formatDateTime(scan.scanned_at)}</td>
								<td class="px-4 py-3 text-right">
									{#if $canDoAction('action.vuln_scan')}
									<!-- svelte-ignore a11y_click_events_have_key_events -->
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<span onclick={(e) => { e.stopPropagation(); scanImage(scan.image); }}>
										<Button size="sm" variant="warning" disabled={scanningImage === scan.image} loading={scanningImage === scan.image} title={$t('vuln.scan')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
										</Button>
									</span>
									{/if}
								</td>
							</tr>

							<!-- Expanded CVE Details -->
							{#if scan.id != null && expandedRows.has(scan.id)}
								{@const cves = parseCves(scan)}
								<tr>
									<td colspan="8" class="px-0 py-0">
										<div class="bg-[var(--bg-hover)] border-b border-[var(--border)]">
											{#if cves.length === 0}
												<div class="px-6 py-4 text-xs text-[var(--text-muted)] text-center">No CVE details available</div>
											{:else}
												{@const showAllCves = scan.id != null && cveShowAll.has(scan.id)}
												{@const visibleCveList = showAllCves ? cves : cves.slice(0, CVE_PAGE_SIZE)}
												<div class="overflow-x-auto">
													<table class="w-full text-xs">
														<thead>
															<tr class="border-b border-[var(--border)]">
																<th class="text-left px-6 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.cveId')}</th>
																<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.severity')}</th>
																<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.package')}</th>
																<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.version')}</th>
																<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.fixedVersion')}</th>
																<th class="text-left px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('vuln.description')}</th>
															</tr>
														</thead>
														<tbody>
															{#each visibleCveList as cve, ci (cve.id + '-' + ci)}
																<tr class="border-b border-[var(--border)] last:border-0">
																	<td class="px-6 py-2 font-mono whitespace-nowrap">
																		{#if cve.id && cve.id.startsWith('CVE-')}
																			<a href="https://nvd.nist.gov/vuln/detail/{cve.id}" target="_blank" rel="noopener" class="text-[var(--accent)] hover:underline">{cve.id}</a>
																		{:else}
																			<span class="text-[var(--text)]">{cve.id || '—'}</span>
																		{/if}
																	</td>
																	<td class="px-3 py-2">
																		<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium {severityBadgeClass(cve.severity)}">
																			{cve.severity}
																		</span>
																	</td>
																	<td class="px-3 py-2 text-[var(--text-secondary)]">{cve.package}</td>
																	<td class="px-3 py-2 text-[var(--text-muted)] font-mono">{cve.version}</td>
																	<td class="px-3 py-2 text-[var(--text-muted)] font-mono">{cve.fixed || '-'}</td>
																	<td class="px-3 py-2 text-[var(--text-secondary)] max-w-[300px] truncate" title={cve.description}>{cve.description || '-'}</td>
																</tr>
															{/each}
														</tbody>
													</table>
													{#if cves.length > CVE_PAGE_SIZE && !showAllCves}
														<div class="px-6 py-2 border-t border-[var(--border)] text-center">
															<button class="text-xs text-[var(--accent)] hover:underline" onclick={() => { if (scan.id != null) { const s = new Set(cveShowAll); s.add(scan.id); cveShowAll = s; } }}>
																Show all {cves.length} CVEs ({cves.length - CVE_PAGE_SIZE} more)
															</button>
														</div>
													{/if}
												</div>
											{/if}
										</div>
									</td>
								</tr>
							{/if}
						{/each}
					</tbody>
				</table>
			</div>

			<Pagination total={scans.length} {page} {perPage} onchange={handlePageChange} />
		{/if}
	</div>
</div>
