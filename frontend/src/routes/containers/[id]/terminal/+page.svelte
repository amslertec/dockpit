<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/stores';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { browser } from '$app/environment';
	import { t } from '$lib/i18n';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import Button from '$lib/components/ui/Button.svelte';

	const containerId = $derived($page.params.id);
	const fromStack = $derived($page.url.searchParams.get('stack'));
	const backHref = $derived(fromStack ? `/stacks/${fromStack}` : '/containers');

	let containerName = $state('');
	let shell = $state('/bin/sh');
	let customShell = $state('');
	let user = $state('');
	let connected = $state(false);
	let connecting = $state(false);
	let error = $state('');
	let terminalEl: HTMLElement;

	let ws: WebSocket | null = null;
	let terminal: any = null;
	let fitAddon: any = null;

	let ctxMenu = $state<{ x: number; y: number; hasSelection: boolean } | null>(null);

	const shells = [
		{ value: '/bin/bash', label: '/bin/bash' },
		{ value: '/bin/sh', label: '/bin/sh' },
		{ value: '/bin/ash', label: '/bin/ash' },
		{ value: '/bin/dash', label: '/bin/dash' },
		{ value: 'custom', label: 'Custom...' },
	];

	onMount(async () => {
		const r = await api.get<any[]>(`/env/${$selectedEnv}/containers`);
		if (r.success && r.data) {
			const c = r.data.find((x: any) => x.id === containerId || x.id.startsWith(containerId));
			if (c) containerName = c.name;
		}
	});

	onDestroy(() => disconnect());

	function closeCtx() { ctxMenu = null; }

	let copySuccess = $state(false);
	let pasteMode = $state(false);

	function copySelection() {
		if (!terminal?.hasSelection()) return;
		const text = terminal.getSelection();
		let copied = false;
		if (navigator.clipboard?.writeText) {
			navigator.clipboard.writeText(text).then(() => { copied = true; }).catch(() => {});
		}
		if (!copied) {
			const ta = document.createElement('textarea');
			ta.value = text;
			ta.style.cssText = 'position:fixed;left:-9999px;opacity:0';
			document.body.appendChild(ta);
			ta.focus();
			ta.select();
			try { copied = document.execCommand('copy'); } catch {}
			document.body.removeChild(ta);
		}
		if (!copied) {
			pasteMode = true;
			requestAnimationFrame(() => {
				const input = document.querySelector('.ctx-paste-input') as HTMLInputElement;
				if (input) { input.value = text; input.select(); }
			});
			return;
		}
		terminal.clearSelection();
		copySuccess = true;
		setTimeout(() => copySuccess = false, 1500);
		ctxMenu = null;
	}

	async function doPaste() {
		ctxMenu = null;
		try {
			const text = await navigator.clipboard.readText();
			if (text && ws) ws.send(text);
		} catch {}
		terminal?.focus();
	}

	function selectAll() { terminal?.selectAll(); ctxMenu = null; }
	function clearTerminal() { terminal?.clear(); ctxMenu = null; }

	async function connect() {
		if (!browser) return;
		error = '';
		connecting = true;
		const selectedShell = shell === 'custom' ? customShell : shell;
		if (!selectedShell) { error = $t('terminal.shellRequired'); connecting = false; return; }

		try {
			const { Terminal } = await import('@xterm/xterm');
			const { FitAddon } = await import('@xterm/addon-fit');
			await import('@xterm/xterm/css/xterm.css');

			const token = localStorage.getItem('dp_token') || '';
			const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
			const url = `${proto}//${location.host}/api/env/${$selectedEnv}/containers/${containerId}/terminal?token=${encodeURIComponent(token)}&shell=${encodeURIComponent(selectedShell)}${user ? `&user=${encodeURIComponent(user)}` : ''}`;

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
							rightClickSelectsWord: false,
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
						terminalEl.addEventListener('contextmenu', (e: MouseEvent) => {
							e.preventDefault(); e.stopPropagation();
							ctxMenu = { x: e.clientX, y: e.clientY, hasSelection: terminal.hasSelection() };
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

			ws.onerror = () => { error = $t('terminal.wsError'); connecting = false; connected = false; };
		} catch (e) { error = `Error: ${(e as Error).message}`; connecting = false; }
	}

	function disconnect() { ws?.close(); ws = null; terminal?.dispose(); terminal = null; fitAddon = null; connected = false; }
	function reconnect() { disconnect(); error = ''; connect(); }
</script>

<svelte:window onclick={closeCtx} />
<svelte:head><title>DockPit — {$t('terminal.title', { name: containerName || containerId })}</title></svelte:head>

<div class="flex flex-col h-[calc(100vh-60px-2rem)]">
	<div class="flex items-center justify-between mb-3 flex-wrap gap-3 shrink-0">
		<div class="flex items-center gap-3">
			<a href={backHref} class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition" aria-label="Back">
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>
			</a>
			<div>
				<h2 class="text-base font-semibold text-[var(--text)]">{$t('terminal.title', { name: containerName || 'Container' })}</h2>
				<div class="text-[10px] font-mono text-[var(--text-muted)]">{containerId}</div>
			</div>
		</div>
		{#if connected}
			<div class="flex items-center gap-2">
				<span class="flex items-center gap-1.5 text-xs text-[var(--green)]">
					<span class="w-2 h-2 rounded-full bg-[var(--green)] animate-pulse"></span>{$t('terminal.connected')}
				</span>
				<Button variant="secondary" size="sm" onclick={disconnect}>{$t('terminal.disconnect')}</Button>
			</div>
		{/if}
	</div>

	<div class="bg-card border border-[var(--border)] rounded-[var(--radius-lg)] p-4 mb-3 shrink-0">
		<div class="flex items-end gap-3 flex-wrap">
			<div class="flex-1 min-w-[120px] max-w-[200px]">
				<label class="block text-[11px] font-medium text-[var(--text-secondary)] mb-1">{$t('terminal.shell')}</label>
				<CustomSelect options={shells} value={shell} onchange={(v) => shell = String(v)} disabled={connected} size="sm" />
			</div>
			{#if shell === 'custom'}
				<div class="flex-1 min-w-[120px] max-w-[200px]">
					<label for="cs" class="block text-[11px] font-medium text-[var(--text-secondary)] mb-1">{$t('terminal.command')}</label>
					<input id="cs" bind:value={customShell} disabled={connected} placeholder="/usr/bin/zsh"
						class="w-full bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs text-[var(--text)] focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200 disabled:opacity-50" />
				</div>
			{/if}
			<div class="flex-1 min-w-[120px] max-w-[200px]">
				<label for="usr" class="block text-[11px] font-medium text-[var(--text-secondary)] mb-1">{$t('terminal.userOptional')}</label>
				<input id="usr" bind:value={user} disabled={connected} placeholder="root"
					class="w-full bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs text-[var(--text)] focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200 disabled:opacity-50" />
			</div>
			{#if !connected}
				<Button variant="primary" size="sm" onclick={connect} loading={connecting} class="h-[30px]">
					{#if !connecting}<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>{/if}
					{connecting ? $t('terminal.connecting') : $t('terminal.connect')}
				</Button>
			{:else}
				<Button variant="secondary" size="sm" onclick={reconnect} class="h-[30px]">
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/></svg>{$t('terminal.reconnect')}
				</Button>
			{/if}
		</div>
		{#if error}
			<div class="mt-2 px-3 py-2 bg-[var(--red-bg)] text-[var(--red)] text-xs rounded-[var(--radius-md)]">{error}</div>
		{/if}
	</div>

	<div class="bg-[#0a0c10] border border-[var(--border)] rounded-[var(--radius-lg)] flex-1 overflow-hidden {connected ? '' : 'flex items-center justify-center'}">
		{#if connected}
			<div bind:this={terminalEl} class="w-full h-full p-1"></div>
		{:else if !connecting}
			<div class="text-center text-[var(--text-muted)] text-sm">
				<svg class="w-10 h-10 mx-auto mb-3 opacity-20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
				<p>{$t('terminal.selectShell')}</p>
				<p class="text-[11px] text-[var(--text-muted)] mt-2">{$t('terminal.rightClickHint')}</p>
			</div>
		{/if}
	</div>
</div>

{#if ctxMenu}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed z-[9999] bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] py-1 min-w-[180px]"
		style="left: {ctxMenu.x}px; top: {ctxMenu.y}px"
		onclick={(e: MouseEvent) => e.stopPropagation()}>
		{#if ctxMenu.hasSelection}
			<button class="w-full flex items-center gap-2.5 px-3 py-1.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition text-left" onclick={copySelection}>
				<svg class="w-3.5 h-3.5 text-[var(--text-secondary)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>
				{$t('terminal.copy')}
			</button>
		{/if}
		<button class="w-full flex items-center gap-2.5 px-3 py-1.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition text-left" onclick={doPaste}>
			<svg class="w-3.5 h-3.5 text-[var(--text-secondary)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2"/><rect x="8" y="2" width="8" height="4" rx="1"/></svg>
			{$t('terminal.paste')}
		</button>
		<div class="border-t border-[var(--border)] my-1"></div>
		<button class="w-full flex items-center gap-2.5 px-3 py-1.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition text-left" onclick={selectAll}>
			<svg class="w-3.5 h-3.5 text-[var(--text-secondary)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M8 12h8"/></svg>
			{$t('terminal.selectAll')}
		</button>
		<button class="w-full flex items-center gap-2.5 px-3 py-1.5 text-xs text-[var(--text)] hover:bg-[var(--bg-hover)] transition text-left" onclick={clearTerminal}>
			<svg class="w-3.5 h-3.5 text-[var(--text-secondary)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/></svg>
			{$t('terminal.clearTerminal')}
		</button>
	</div>
{/if}
