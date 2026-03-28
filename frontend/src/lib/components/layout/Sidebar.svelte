<script lang="ts">
	import { page } from '$app/stores';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { auth, canManageUsers, canManageDocker } from '$lib/stores/auth';
	import { t } from '$lib/i18n';

	interface Props { open: boolean; onclose: () => void; }
	let { open, onclose }: Props = $props();

	const envName = $derived($environments.find(e => e.id === $selectedEnv)?.name || '');

	const navItems = [
		{ href: '/dashboard', key: 'nav.dashboard', icon: 'dash' },
		{ href: '/monitoring', key: 'monitoring.title', icon: 'monitor' },
		{ href: '/stacks', key: 'nav.stacks', icon: 'stack' },
		{ href: '/containers', key: 'nav.containers', icon: 'box' },
		{ href: '/images', key: 'nav.images', icon: 'img' },
		{ href: '/volumes', key: 'nav.volumes', icon: 'vol' },
		{ href: '/networks', key: 'nav.networks', icon: 'net' },
	];

	function isActive(href: string): boolean {
		if (href === '/') return $page.url.pathname === '/';
		return $page.url.pathname === href || $page.url.pathname.startsWith(href + '/');
	}
</script>

{#if open}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/60 backdrop-blur-sm z-[99] md:hidden" onclick={onclose}></div>
{/if}

<aside class="fixed md:static inset-y-0 left-0 z-[100] w-[240px] shrink-0 flex flex-col
	transition-transform duration-300 ease-out {open ? 'translate-x-0' : '-translate-x-full md:translate-x-0'}
	sidebar-glass">

	<a href="/" class="flex items-center gap-3 px-5 h-[60px] border-b border-[var(--border)] shrink-0" onclick={onclose}>
		<div class="w-8 h-8 rounded-[var(--radius-md)] bg-gradient-to-br from-[var(--accent)] to-[#00b4d8] flex items-center justify-center shadow-[var(--shadow-glow)]">
			<img src="/logo.svg" alt="DockPit" class="w-5 h-5 brightness-200" />
		</div>
		<span class="text-[15px] font-bold bg-gradient-to-r from-[var(--accent)] via-[#bf8aff] to-[#00b4d8] bg-clip-text text-transparent">DockPit</span>
	</a>

	<nav class="flex-1 px-3 py-4 overflow-y-auto space-y-5">
		<div>
			<div class="text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)] px-3 mb-2">{$t('nav.start')}</div>
			<a href="/" class="nav-item {isActive('/') ? 'nav-active' : ''}" onclick={onclose}>
				<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
				{$t('nav.home')}
			</a>
		</div>

		{#if $selectedEnv}
			<div>
				<div class="text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)] px-3 mb-2">{envName}</div>
				{#each navItems as item}
					<a href={item.href} class="nav-item {isActive(item.href) ? 'nav-active' : ''}" onclick={onclose}>
						{#if item.icon === 'stack'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 2L2 7l10 5 10-5-10-5z"/><path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/></svg>
						{:else if item.icon === 'dash'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/></svg>
						{:else if item.icon === 'monitor'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>
						{:else if item.icon === 'box'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/></svg>
						{:else if item.icon === 'img'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="2" width="20" height="20" rx="3"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="M21 15l-5-5L5 21"/></svg>
						{:else if item.icon === 'vol'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
						{:else if item.icon === 'net'}
							<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="5" r="3"/><circle cx="5" cy="19" r="3"/><circle cx="19" cy="19" r="3"/><path d="M12 8v4m-4.5 3.5L10 13m4 0l2.5 2.5"/></svg>
						{/if}
						{$t(item.key)}
					</a>
				{/each}
			</div>
		{/if}

		{#if $canManageDocker}
			<div>
				<div class="text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)] px-3 mb-2">{$t('nav.management')}</div>
				<a href="/updates" class="nav-item {isActive('/updates') ? 'nav-active' : ''}" onclick={onclose}>
					<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
					{$t('nav.updates')}
				</a>
				<a href="/events" class="nav-item {isActive('/events') ? 'nav-active' : ''}" onclick={onclose}>
					<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 8v4l3 3"/><circle cx="12" cy="12" r="10"/></svg>
					{$t('events.title')}
				</a>
				<a href="/vulnerabilities" class="nav-item {isActive('/vulnerabilities') ? 'nav-active' : ''}" onclick={onclose}>
					<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
					{$t('vuln.title')}
				</a>
				<a href="/environments" class="nav-item {isActive('/environments') ? 'nav-active' : ''}" onclick={onclose}>
					<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><circle cx="6" cy="6" r="1"/><circle cx="6" cy="18" r="1"/></svg>
					{$t('nav.environments')}
				</a>
				{#if $canManageUsers}
					<a href="/users" class="nav-item {isActive('/users') ? 'nav-active' : ''}" onclick={onclose}>
						<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
						{$t('nav.users')}
					</a>
				{/if}
				<a href="/settings" class="nav-item {isActive('/settings') ? 'nav-active' : ''}" onclick={onclose}>
					<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
					{$t('nav.settings')}
				</a>
			</div>
		{/if}

		<div>
			<div class="text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)] px-3 mb-2">{$t('nav.account')}</div>
			<a href="/profile" class="nav-item {isActive('/profile') ? 'nav-active' : ''}" onclick={onclose}>
				<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
				{$t('nav.profile')}
			</a>
		</div>
	</nav>

	<div class="p-3 border-t border-[var(--border)]">
		<button class="nav-item w-full text-[var(--red)] hover:!bg-[var(--red-bg)]"
			onclick={() => { auth.logout(); window.location.href = '/login'; }}>
			<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
			{$t('nav.logout')}
		</button>
	</div>
</aside>

<style>
	.sidebar-glass {
		background: var(--glass-bg);
		backdrop-filter: blur(24px) saturate(180%);
		-webkit-backdrop-filter: blur(24px) saturate(180%);
		border-right: 1px solid var(--border);
	}
	.nav-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 9px 12px;
		border-radius: var(--radius-md);
		font-size: 13px;
		font-weight: 500;
		color: var(--text-secondary);
		transition: all 0.2s ease;
		text-decoration: none;
	}
	.nav-item:hover {
		background: var(--bg-hover);
		color: var(--text);
	}
	.nav-active {
		background: var(--accent-bg) !important;
		color: var(--accent) !important;
		box-shadow: inset 3px 0 0 var(--accent);
	}
</style>
