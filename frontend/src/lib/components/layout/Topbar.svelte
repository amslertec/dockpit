<script lang="ts">
	import { theme } from '$lib/stores/theme';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { t } from '$lib/i18n';

	interface Props {
		title: string;
		ontoggle: () => void;
	}
	let { title, ontoggle }: Props = $props();

	let dropdownOpen = $state(false);

	const currentEnv = $derived($environments.find((e) => e.id === $selectedEnv));
	const currentEnvName = $derived(currentEnv?.name || '');

	function selectEnv(id: string) {
		selectedEnv.select(id);
		dropdownOpen = false;
	}
</script>

<svelte:window onclick={() => dropdownOpen = false} />

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
