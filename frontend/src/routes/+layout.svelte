<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { auth } from '$lib/stores/auth';
	import { theme } from '$lib/stores/theme';
	import { t } from '$lib/i18n';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { api } from '$lib/api/client';
	import type { EnvironmentInfo, AppStatus } from '$lib/api/types';
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import Topbar from '$lib/components/layout/Topbar.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import CommandPalette from '$lib/components/ui/CommandPalette.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	let ready = $state(false);
	let sidebarOpen = $state(false);
	let envsLoaded = $state(false);
	let showPalette = $state(false);

	function handleGlobalKeydown(e: KeyboardEvent) {
		if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
			e.preventDefault();
			showPalette = !showPalette;
		}
	}

	const publicRoutes = ['/login', '/setup'];
	const isPublic = $derived(publicRoutes.some((r) => $page.url.pathname.startsWith(r)));

	const titleKeys: Record<string, string> = {
		'/': 'nav.home',
		'/dashboard': 'nav.dashboard',
		'/containers': 'nav.containers',
		'/images': 'nav.images',
		'/volumes': 'nav.volumes',
		'/networks': 'nav.networks',
		'/stacks': 'nav.stacks',
		'/stacks/new': 'stacks.newStack',
		'/environments': 'nav.environments',
		'/users': 'nav.users',
		'/settings': 'nav.settings',
		'/monitoring': 'monitoring.title',
		'/updates': 'updates.title',
		'/events': 'events.title',
		'/vulnerabilities': 'vuln.title',
		'/audit': 'audit.title',
		'/health': 'health.title',
		'/profile': 'nav.profile',
		'/host-terminal': 'hostTerminal.title',
		'/dependencies': 'nav.dependencies'
	};
	const pageTitle = $derived(titleKeys[$page.url.pathname] ? $t(titleKeys[$page.url.pathname]) : 'DockPit');

	async function loadEnvironments() {
		if (envsLoaded) return;
		const token = $auth.token || localStorage.getItem('dp_token');
		if (!token) return;

		try {
			const r = await api.get<EnvironmentInfo[]>('/environments');
			if (r.success && r.data) {
				environments.set(r.data);
				const savedEnv = $selectedEnv;
				const envExists = r.data.some(e => e.id === savedEnv);
				if (!savedEnv || !envExists) {
					if (r.data.length > 0) selectedEnv.select(r.data[0].id);
				}
				envsLoaded = true;

				// Check status of remote servers in background
				for (const env of r.data) {
					if (!env.is_local) {
						api.get<string>(`/environments/${env.id}/status`).then(sr => {
							if (sr.success && sr.data) {
								environments.update(list =>
									list.map(e => e.id === env.id ? { ...e, status: sr.data! } : e)
								);
							}
						});
					}
				}
			}
		} catch {}
	}

	onMount(async () => {
		theme.init();

		const hasToken = $auth.token || localStorage.getItem('dp_token');
		const currentPath = $page.url.pathname;

		// 1. Check API status — retry until backend is ready (handles slow Docker startup)
		let status: Awaited<ReturnType<typeof api.get<AppStatus>>> | null = null;
		for (let attempt = 0; attempt < 20; attempt++) {
			status = await api.get<AppStatus>('/status');
			if (status.success) break;
			await new Promise(r => setTimeout(r, 1000));
		}

		// 2. Route decision based on setup state
		if (status?.success) {
			const done = status.data?.setup_complete;
			if (!done) {
				// Setup not complete → must go to setup
				if (currentPath !== '/setup') goto('/setup');
				ready = true;
				return;
			}
			if (!hasToken && !isPublic) {
				goto('/login');
				ready = true;
				return;
			}
		} else {
			// API still unreachable after retries — redirect based on token
			if (!hasToken) {
				goto('/login');
				ready = true;
				return;
			}
		}

		// 3. If on /login or /setup with a valid token, redirect to the app
		if (hasToken && (currentPath === '/login' || currentPath === '/setup')) {
			goto('/');
			ready = true;
			return;
		}

		// 4. Load permissions, then environments, then show the current page
		const permR = await api.get<{role: string; permissions: string[]}>('/my-permissions');
		if (permR.success && permR.data) {
			auth.setPermissions(permR.data.permissions);
		}
		await loadEnvironments();
		ready = true;
	});

	// Poll permissions every 5 minutes (picks up group changes without re-login)
	let permInterval: ReturnType<typeof setInterval> | undefined;
	$effect(() => {
		if ($auth.token && ready) {
			if (permInterval) clearInterval(permInterval);
			permInterval = setInterval(async () => {
				const r = await api.get<{role: string; permissions: string[]}>('/my-permissions');
				if (r.success && r.data) auth.setPermissions(r.data.permissions);
			}, 300000); // 5 minutes
		}
		return () => { if (permInterval) clearInterval(permInterval); };
	});

	// Reactive fallback: if environments weren't loaded yet (e.g. timing issue),
	// try again whenever auth token becomes available
	$effect(() => {
		if ($auth.token && !envsLoaded && ready) {
			loadEnvironments();
		}
	});
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<Toast />

{#if !ready}
	<div class="flex items-center justify-center h-screen bg-0">
		<div class="flex flex-col items-center gap-4">
			<div class="w-7 h-7 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
			<p class="text-[var(--text-secondary)] text-sm">DockPit</p>
		</div>
	</div>
{:else if isPublic}
	{@render children()}
{:else}
	<div class="flex h-screen bg-0">
		<Sidebar open={sidebarOpen} onclose={() => (sidebarOpen = false)} />
		<div class="flex-1 flex flex-col min-w-0">
			<div class="relative z-50">
				<Topbar title={pageTitle} ontoggle={() => (sidebarOpen = !sidebarOpen)} onsearch={() => showPalette = true} />
			</div>
			<main class="flex-1 overflow-y-auto p-4 md:p-5">
				{@render children()}
			</main>
		</div>
	</div>
{/if}

{#if showPalette}
	<CommandPalette onclose={() => showPalette = false} />
{/if}
