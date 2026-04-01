<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { api } from '$lib/api/client';
	import { environments } from '$lib/stores/environment';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import type { EnvironmentInfo } from '$lib/api/types';

	let selectedServer = $state('');
	let connected = $state(false);
	let connecting = $state(false);
	let error = $state('');
	let terminalEl: HTMLElement;

	let ws: WebSocket | null = null;
	let terminal: any = null;
	let fitAddon: any = null;

	onDestroy(() => disconnect());

	function disconnect() {
		ws?.close();
		ws = null;
		terminal?.dispose();
		terminal = null;
		fitAddon = null;
		connected = false;
	}

	function serverName(id: string): string {
		return $environments.find(e => e.id === id)?.name || id;
	}

	async function connect() {
		if (!selectedServer) return;
		error = '';
		connecting = true;

		try {
			const { Terminal } = await import('@xterm/xterm');
			const { FitAddon } = await import('@xterm/addon-fit');
			await import('@xterm/xterm/css/xterm.css');

			const token = await api.getWsToken();
			const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
			const url = `${proto}//${location.host}/api/env/${selectedServer}/host-terminal?token=${encodeURIComponent(token)}`;

			ws = new WebSocket(url);

			ws.onopen = () => {
				connecting = false;
				connected = true;
				requestAnimationFrame(() => {
					requestAnimationFrame(() => {
						if (!terminalEl) { error = 'Terminal element not found'; return; }
						terminal = new Terminal({
							cursorBlink: true, fontSize: 14,
							fontFamily: "'SF Mono', 'Fira Code', 'Cascadia Code', monospace",
							theme: {
								background: '#0a0c10', foreground: '#e4e7ee', cursor: '#6c5ce7',
								selectionBackground: 'rgba(108,92,231,0.3)',
								black: '#1a1f2b', red: '#ff5c75', green: '#00e88f', yellow: '#ffbe2e',
								blue: '#6c5ce7', magenta: '#bf8aff', cyan: '#22d3ee', white: '#e4e7ee',
								brightBlack: '#4e5678', brightRed: '#ff7b7b', brightGreen: '#5ef0c0',
								brightYellow: '#ffe066', brightBlue: '#8577ed', brightMagenta: '#c4a8ff',
								brightCyan: '#5ef0e0', brightWhite: '#ffffff',
							}
						});
						fitAddon = new FitAddon();
						terminal.loadAddon(fitAddon);
						terminal.open(terminalEl);
						fitAddon.fit();
						ws?.send(JSON.stringify({ type: 'resize', cols: terminal.cols, rows: terminal.rows }));
						terminal.onData((data: string) => ws?.send(data));
						terminal.onResize(({ cols, rows }: { cols: number; rows: number }) => {
							ws?.send(JSON.stringify({ type: 'resize', cols, rows }));
						});
						new ResizeObserver(() => fitAddon?.fit()).observe(terminalEl);
					});
				});
			};

			ws.onmessage = (event) => {
				if (event.data instanceof Blob) {
					event.data.text().then((text: string) => terminal?.write(text));
				} else { terminal?.write(event.data); }
			};

			ws.onclose = () => {
				connecting = false;
				if (connected) {
					connected = false;
					terminal?.write(`\r\n\x1b[90m${$t('terminal.disconnected')}\x1b[0m\r\n`);
				} else { error = $t('terminal.connectionFailed'); }
			};

			ws.onerror = () => { connecting = false; error = $t('terminal.connectionFailed'); };
		} catch (e) {
			connecting = false;
			error = String(e);
		}
	}
</script>

<svelte:head><title>DockPit — {$t('hostTerminal.title')}</title></svelte:head>

<div class="space-y-4">
	<div class="flex items-center justify-between flex-wrap gap-3">
		<div>
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('hostTerminal.title')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{$t('hostTerminal.desc')}</p>
		</div>
	</div>

	<div class:hidden={connected}>
		<div class="bg-card border border-theme rounded-lg p-5">
			<h3 class="text-sm font-semibold text-primary mb-3">{$t('hostTerminal.selectServer')}</h3>
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 mb-4">
				{#each $environments as env}
					<button
						class="flex items-center gap-3 p-4 rounded-xl border-2 transition-all duration-200 text-left
						{selectedServer === env.id ? 'border-[var(--accent)] bg-[var(--accent-bg)] shadow-[0_0_16px_-4px_var(--accent)]' : 'border-[var(--border)] bg-[var(--bg-0)] hover:border-[var(--border-light)]'}"
						onclick={() => selectedServer = env.id}
					>
						<div class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0 {env.status === 'online' || env.is_local ? 'bg-[var(--green-bg)]' : 'bg-[var(--red-bg)]'}">
							<svg class="w-5 h-5 {env.status === 'online' || env.is_local ? 'text-[var(--green)]' : 'text-[var(--red)]'}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
						</div>
						<div class="min-w-0 flex-1">
							<div class="text-sm font-medium text-primary truncate">{env.name}</div>
							<div class="text-[11px] text-muted">{env.is_local ? 'Local' : env.url}</div>
						</div>
						<span class="w-2.5 h-2.5 rounded-full shrink-0 {env.status === 'online' || env.is_local ? 'bg-[var(--green)] shadow-[var(--shadow-glow-green)]' : 'bg-[var(--red)]'}"></span>
					</button>
				{/each}
			</div>

			{#if error}
				<p class="text-[var(--red)] text-xs mb-3">{error}</p>
			{/if}

			<Button variant="success" size="md" onclick={connect} loading={connecting} disabled={!selectedServer}>
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
				{connecting ? $t('hostTerminal.connecting') : $t('hostTerminal.connect')}
			</Button>
		</div>
	</div>

	<div class:hidden={!connected}>
		<div class="bg-card border border-theme rounded-lg overflow-hidden">
			<div class="flex items-center justify-between px-4 py-2.5 border-b border-theme bg-[var(--bg-1)]">
				<div class="flex items-center gap-2">
					<span class="w-2 h-2 rounded-full bg-[var(--green)] animate-pulse"></span>
					<span class="text-xs font-medium text-primary">{serverName(selectedServer)}</span>
					<span class="text-[10px] text-muted">— {$t('hostTerminal.hostShell')}</span>
				</div>
				<Button variant="danger" size="sm" onclick={disconnect}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					{$t('terminal.disconnect')}
				</Button>
			</div>
			<div bind:this={terminalEl} class="w-full" style="height: calc(100vh - 220px); min-height: 400px;"></div>
		</div>
	</div>
</div>
