<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import Badge from './Badge.svelte';
	import type { ContainerInfo, StackInfo } from '$lib/api/types';

	interface Props { onclose: () => void; }
	let { onclose }: Props = $props();

	let query = $state('');
	let activeIndex = $state(0);
	let inputEl: HTMLInputElement | undefined = $state();
	let containers = $state<ContainerInfo[]>([]);
	let stacks = $state<StackInfo[]>([]);
	let loading = $state(true);

	interface ResultItem {
		id: string;
		label: string;
		sublabel?: string;
		category: string;
		icon: string;
		action: () => void;
		badge?: { status: string };
	}

	const pages: ResultItem[] = [
		{ id: 'p-home', label: 'Home', sublabel: '/', category: 'pages', icon: 'home', action: () => nav('/') },
		{ id: 'p-dash', label: 'Dashboard', sublabel: '/dashboard', category: 'pages', icon: 'grid', action: () => nav('/dashboard') },
		{ id: 'p-monitoring', label: 'Monitoring', sublabel: '/monitoring', category: 'pages', icon: 'monitor', action: () => nav('/monitoring') },
		{ id: 'p-containers', label: 'Containers', sublabel: '/containers', category: 'pages', icon: 'box', action: () => nav('/containers') },
		{ id: 'p-stacks', label: 'Stacks', sublabel: '/stacks', category: 'pages', icon: 'layers', action: () => nav('/stacks') },
		{ id: 'p-images', label: 'Images', sublabel: '/images', category: 'pages', icon: 'image', action: () => nav('/images') },
		{ id: 'p-volumes', label: 'Volumes', sublabel: '/volumes', category: 'pages', icon: 'database', action: () => nav('/volumes') },
		{ id: 'p-networks', label: 'Networks', sublabel: '/networks', category: 'pages', icon: 'network', action: () => nav('/networks') },
		{ id: 'p-env', label: 'Environments', sublabel: '/environments', category: 'pages', icon: 'server', action: () => nav('/environments') },
		{ id: 'p-updates', label: 'Updates', sublabel: '/updates', category: 'pages', icon: 'download', action: () => nav('/updates') },
		{ id: 'p-users', label: 'Users', sublabel: '/users', category: 'pages', icon: 'users', action: () => nav('/users') },
		{ id: 'p-settings', label: 'Settings', sublabel: '/settings', category: 'pages', icon: 'settings', action: () => nav('/settings') },
		{ id: 'p-profile', label: 'Profile', sublabel: '/profile', category: 'pages', icon: 'user', action: () => nav('/profile') },
	];

	const serverItems = $derived(
		$environments.map(env => ({
			id: `s-${env.id}`,
			label: env.name,
			sublabel: env.is_local ? 'Local' : env.url,
			category: 'servers',
			icon: 'server',
			badge: { status: env.status === 'online' || env.is_local ? 'online' : 'offline' },
			action: () => { selectedEnv.select(env.id); nav('/dashboard'); },
		} as ResultItem))
	);

	const containerItems = $derived(
		containers.map(c => ({
			id: `c-${c.id}`,
			label: c.name,
			sublabel: c.image,
			category: 'containers',
			icon: 'box',
			badge: { status: c.state },
			action: () => nav(`/containers`),
		} as ResultItem))
	);

	const stackItems = $derived(
		stacks.map(s => ({
			id: `st-${s.name}`,
			label: s.name,
			sublabel: `${s.running_services}/${s.services_count} services`,
			category: 'stacks',
			icon: 'layers',
			badge: { status: s.status },
			action: () => nav(`/stacks/${s.name}`),
		} as ResultItem))
	);

	const allItems = $derived([...pages, ...serverItems, ...containerItems, ...stackItems]);

	const filtered = $derived(() => {
		const q = query.toLowerCase().trim();
		if (!q) return pages.slice(0, 8);

		const results: ResultItem[] = [];
		const categories = [
			{ items: pages, max: 4 },
			{ items: serverItems, max: 3 },
			{ items: containerItems, max: 5 },
			{ items: stackItems, max: 5 },
		];
		for (const cat of categories) {
			const matches = cat.items.filter(item =>
				item.label.toLowerCase().includes(q) ||
				(item.sublabel || '').toLowerCase().includes(q)
			);
			results.push(...matches.slice(0, cat.max));
		}
		return results.slice(0, 20);
	});

	function nav(path: string) {
		onclose();
		goto(path);
	}

	function handleKeydown(e: KeyboardEvent) {
		const items = filtered();
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			activeIndex = (activeIndex + 1) % Math.max(items.length, 1);
			scrollToActive();
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			activeIndex = (activeIndex - 1 + Math.max(items.length, 1)) % Math.max(items.length, 1);
			scrollToActive();
		} else if (e.key === 'Enter') {
			e.preventDefault();
			if (items[activeIndex]) items[activeIndex].action();
		} else if (e.key === 'Escape') {
			onclose();
		}
	}

	function scrollToActive() {
		requestAnimationFrame(() => {
			document.querySelector(`[data-palette-index="${activeIndex}"]`)?.scrollIntoView({ block: 'nearest' });
		});
	}

	$effect(() => { query; activeIndex = 0; });

	onMount(async () => {
		inputEl?.focus();
		if ($selectedEnv) {
			const [cr, sr] = await Promise.all([
				api.get<ContainerInfo[]>(`/env/${$selectedEnv}/containers`),
				api.get<StackInfo[]>(`/env/${$selectedEnv}/stacks`),
			]);
			if (cr.success && cr.data) containers = cr.data;
			if (sr.success && sr.data) stacks = sr.data;
		}
		loading = false;
	});

	function categoryLabel(cat: string): string {
		const map: Record<string, string> = {
			pages: $t('palette.pages'),
			servers: $t('palette.servers'),
			containers: $t('palette.containers'),
			stacks: $t('palette.stacks'),
		};
		return map[cat] || cat;
	}

	function categoryIcon(cat: string): string {
		const map: Record<string, string> = { pages: '📄', servers: '🖥', containers: '📦', stacks: '🗂' };
		return map[cat] || '•';
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-[99999] flex items-start justify-center pt-[15vh] palette-backdrop"
	onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
	<div class="w-full max-w-[560px] mx-4 palette-content" onkeydown={handleKeydown}>
		<!-- Search input -->
		<div class="flex items-center gap-3 px-4 py-3.5 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-t-[var(--radius-xl)]">
			<svg class="w-5 h-5 text-[var(--text-muted)] shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
			<input
				bind:this={inputEl}
				bind:value={query}
				placeholder={$t('palette.title')}
				class="flex-1 bg-transparent text-[var(--text)] text-sm placeholder:text-[var(--text-muted)] outline-none"
			/>
			<kbd class="hidden sm:inline-flex items-center px-1.5 py-0.5 text-[10px] text-[var(--text-muted)] border border-[var(--border)] rounded-[var(--radius-sm)] font-mono">ESC</kbd>
		</div>

		<!-- Results -->
		<div class="bg-[var(--dropdown-bg)] border border-t-0 border-[var(--border-light)] rounded-b-[var(--radius-xl)] max-h-[50vh] overflow-y-auto">
			{#if loading}
				<div class="flex items-center justify-center py-8">
					<div class="w-5 h-5 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
				</div>
			{:else}
				{@const items = filtered()}
				{#if items.length === 0}
					<div class="px-4 py-8 text-center text-sm text-[var(--text-muted)]">{$t('palette.noResults')}</div>
				{:else}
					{#each items as item, i}
						{@const showHeader = i === 0 || items[i - 1].category !== item.category}
						{#if showHeader}
							<div class="px-4 pt-3 pb-1 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)]">
								{categoryLabel(item.category)}
							</div>
						{/if}
						<button
							data-palette-index={i}
							class="w-full flex items-center gap-3 px-4 py-2.5 text-left transition-all duration-75
							{i === activeIndex ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text)]'}"
							onclick={() => item.action()}
							onmouseenter={() => activeIndex = i}
						>
							<span class="text-sm w-5 text-center shrink-0">{categoryIcon(item.category)}</span>
							<div class="flex-1 min-w-0">
								<div class="text-sm font-medium truncate {i === activeIndex ? 'text-[var(--accent)]' : 'text-[var(--text)]'}">{item.label}</div>
								{#if item.sublabel}
									<div class="text-[11px] text-[var(--text-muted)] truncate">{item.sublabel}</div>
								{/if}
							</div>
							{#if item.badge}
								<Badge status={item.badge.status} />
							{/if}
						</button>
					{/each}
				{/if}
			{/if}

			<!-- Hint -->
			<div class="px-4 py-2 border-t border-[var(--border)] text-[10px] text-[var(--text-muted)] flex items-center gap-3">
				<span>{$t('palette.hint')}</span>
				<span class="ml-auto opacity-50">Ctrl+K</span>
			</div>
		</div>
	</div>
</div>

<style>
	.palette-backdrop {
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
		animation: palette-bg 0.15s ease-out;
	}
	.palette-content {
		animation: palette-slide 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes palette-bg {
		from { opacity: 0; }
		to { opacity: 1; }
	}
	@keyframes palette-slide {
		from { opacity: 0; transform: translateY(-20px) scale(0.97); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}
</style>
