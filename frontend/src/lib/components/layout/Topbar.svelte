<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { theme } from '$lib/stores/theme';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { t, locale } from '$lib/i18n';
	import { api } from '$lib/api/client';
	import NotificationPanel from '$lib/components/ui/NotificationPanel.svelte';

	interface Props {
		title: string;
		ontoggle: () => void;
		onsearch?: () => void;
	}
	let { title, ontoggle, onsearch }: Props = $props();

	let dropdownOpen = $state(false);
	let showNotifications = $state(false);
	let unreadCount = $state(0);

	const currentEnv = $derived($environments.find((e) => e.id === $selectedEnv));
	const currentEnvName = $derived(currentEnv?.name || '');

	function selectEnv(id: string) {
		selectedEnv.select(id);
		dropdownOpen = false;
	}

	async function fetchUnreadCount() {
		const res = await api.get<{ count: number }>('/notifications/unread-count');
		if (res.success && res.data) {
			unreadCount = res.data.count;
		}
	}

	let pollInterval: ReturnType<typeof setInterval>;

	onMount(() => {
		fetchUnreadCount();
		pollInterval = setInterval(fetchUnreadCount, 30000);
	});

	onDestroy(() => {
		if (pollInterval) clearInterval(pollInterval);
	});
</script>

<svelte:window onclick={() => { dropdownOpen = false; showNotifications = false; }} />

<header class="h-[60px] border-b border-[var(--border)] flex items-center justify-between px-4 md:px-6 shrink-0 topbar-glass">
	<div class="flex items-center gap-3 min-w-0">
		<button class="md:hidden text-[var(--text)] p-1.5 rounded-[var(--radius-md)] hover:bg-[var(--bg-hover)] transition-all" aria-label={$t('topbar.menu')} onclick={ontoggle}>
			<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
		</button>
		<h1 class="text-[15px] font-semibold text-[var(--text)] truncate">{title}</h1>
		{#if $selectedEnv}
			<span class="text-xs text-[var(--text-muted)] hidden md:inline">— {currentEnvName}</span>
		{/if}
	</div>

	<div class="flex items-center gap-3">
		{#if $environments.length > 0}
			<div class="relative">
				<button
					class="flex items-center gap-2 bg-[var(--bg-3)] border border-[var(--border)] rounded-[var(--radius-md)] px-3.5 py-2 text-xs text-[var(--text)] hover:border-[var(--border-light)] hover:shadow-[var(--shadow-sm)] transition-all duration-200 min-w-[130px] max-w-[220px]"
					onclick={(e: MouseEvent) => { e.stopPropagation(); dropdownOpen = !dropdownOpen; }}
				>
					<span class="w-2 h-2 rounded-full shrink-0 {currentEnv?.status === 'online' || currentEnv?.is_local ? 'bg-[var(--green)] shadow-[var(--shadow-glow-green)]' : 'bg-[var(--red)]'}"></span>
					<span class="truncate font-medium">{currentEnvName}</span>
					<svg class="w-3.5 h-3.5 text-[var(--text-muted)] shrink-0 transition-transform duration-200 {dropdownOpen ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
				</button>

				{#if dropdownOpen}
					<div class="fixed right-4 mt-2 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] z-[9999] min-w-[220px] py-1.5 overflow-hidden env-dropdown-enter" style="top: 56px;">
						<div class="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)]">{$t('topbar.selectEnv')}</div>
						{#each $environments as env}
							<button
								class="w-full flex items-center gap-3 px-3 py-2.5 text-xs text-left transition-all duration-150
								{env.id === $selectedEnv ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text)]'}"
								onclick={() => selectEnv(env.id)}
							>
								<span class="w-2 h-2 rounded-full shrink-0 {env.status === 'online' || env.is_local ? 'bg-[var(--green)]' : 'bg-[var(--red)]'}"></span>
								<span class="truncate font-medium">{env.name}</span>
								{#if env.is_local}
									<span class="text-[9px] text-[var(--text-muted)] ml-auto px-1.5 py-0.5 rounded-full bg-[var(--bg-3)]">{$t('topbar.local')}</span>
								{/if}
								{#if env.id === $selectedEnv}
									<svg class="w-3.5 h-3.5 shrink-0 ml-auto text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
								{/if}
							</button>
						{/each}
					</div>
				{/if}
			</div>
		{/if}

		<!-- Search (Ctrl+K) -->
		<button class="w-9 h-9 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all duration-200" aria-label="Search" title="Search (Ctrl+K)" onclick={onsearch}>
			<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
		</button>

		<!-- Notifications -->
		<div class="relative">
			<button class="w-9 h-9 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all duration-200 relative"
				aria-label="Notifications" onclick={(e) => { e.stopPropagation(); showNotifications = !showNotifications; }}>
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 01-3.46 0"/></svg>
				{#if unreadCount > 0}
					<span class="absolute -top-1 -right-1 w-4 h-4 bg-[var(--red)] text-white text-[9px] font-bold rounded-full flex items-center justify-center">{unreadCount > 9 ? '9+' : unreadCount}</span>
				{/if}
			</button>
			{#if showNotifications}
				<NotificationPanel onclose={() => showNotifications = false} />
			{/if}
		</div>

		<!-- Language toggle with flags -->
		<button class="w-9 h-9 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] hover:border-[var(--border-light)] transition-all duration-200 text-base leading-none" aria-label="Language" onclick={() => locale.toggle()} title={$locale === 'en' ? 'Deutsch' : 'English'}>
			{#if $locale === 'en'}
				<svg class="w-5 h-5" viewBox="0 0 36 36"><path fill="#FFCD05" d="M0 27a4 4 0 004 4h28a4 4 0 004-4v-3H0v3z"/><path fill="#ED1F24" d="M0 12h36v12H0z"/><path fill="#141414" d="M32 5H4a4 4 0 00-4 4v3h36V9a4 4 0 00-4-4z"/></svg>
			{:else}
				<svg class="w-5 h-5" viewBox="0 0 36 36"><path fill="#00247D" d="M32 5H4a4 4 0 00-4 4v18a4 4 0 004 4h28a4 4 0 004-4V9a4 4 0 00-4-4z"/><path fill="#CF1B2B" d="M22.6 13.5L36 7.3V9l-9.7 4.5h-3.7zM36 27l-14.2-6.5h3.7L36 25v2zM0 9v-1.7l14.2 6.5H10.5L0 9.3V9zM0 27v-2l10.5-4.5h3.7L0 27z"/><path fill="#EEE" d="M36 14.5v7H20.5V31h-5V21.5H0v-7h15.5V5h5v9.5z"/><path fill="#CF1B2B" d="M36 16v4H19v11h-2V20H0v-4h17V5h2v11z"/></svg>
			{/if}
		</button>

		<!-- Theme toggle -->
		<button class="w-9 h-9 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] hover:shadow-[var(--shadow-glow)] transition-all duration-300" aria-label={$t('topbar.theme')} onclick={() => theme.toggle()}>
			{#if $theme === 'dark'}
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
			{:else}
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>
			{/if}
		</button>
	</div>
</header>

<style>
	.topbar-glass {
		background: var(--glass-bg);
		backdrop-filter: blur(24px) saturate(180%);
		-webkit-backdrop-filter: blur(24px) saturate(180%);
	}
	.env-dropdown-enter {
		animation: env-open 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes env-open {
		from { opacity: 0; transform: translateY(-8px) scale(0.96); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}
</style>
