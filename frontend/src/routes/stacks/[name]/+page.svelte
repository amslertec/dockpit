<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Badge from '$lib/components/ui/Badge.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import YAML from 'yaml';
	import { formatPorts, formatDate, extractHealth } from '$lib/utils/format';
	import type { StackDetail, StackFile, ImageUpdateCheck } from '$lib/api/types';

	const stackName = $derived($page.params.name);

	let detail = $state<StackDetail | null>(null);
	let loading = $state(true);
	let activeTab = $state(0);
	let saving = $state(false);
	let yamlError = $state('');
	let output = $state('');
	let showEditor = $state(false);
	let confirm = $state<{ message: string; action: () => void } | null>(null);
	interface RecreateStep { text: string; status: 'pending' | 'running' | 'done' | 'error'; detail?: string; }
	let recreateModal = $state<{ name: string; image: string; steps: RecreateStep[]; output: string; done: boolean } | null>(null);
	let updateStatus = $state<Map<string, 'checking' | 'up-to-date' | 'outdated'>>(new Map());

	function isChecking(id: string) { return updateStatus.get(id) === 'checking'; }
	function getStatus(id: string) { return updateStatus.get(id); }

	async function checkUpdatesInBackground() {
		if (!detail) return;
		for (const c of detail.containers) {
			if (c.state === 'running') {
				const m = new Map(updateStatus); m.set(c.id, 'checking'); updateStatus = m;
				const r = await api.post<ImageUpdateCheck>(`/env/${$selectedEnv}/containers/${c.id}/check-update`, {});
				const m2 = new Map(updateStatus);
				if (r.success && r.data) m2.set(c.id, r.data.outdated ? 'outdated' : 'up-to-date');
				else m2.delete(c.id);
				updateStatus = m2;
			}
		}
	}

	interface Tab { name: string; content: string; removable: boolean; }
	let tabs = $state<Tab[]>([]);

	onMount(() => load());

	async function load() {
		if (!$selectedEnv || !stackName) return;
		loading = true;
		const r = await api.get<StackDetail>(`/env/${$selectedEnv}/stacks/${stackName}`);
		if (r.success && r.data) {
			detail = r.data;
			checkUpdatesInBackground();
			tabs = [
				{ name: 'docker-compose.yml', content: r.data.compose_content, removable: false },
				{ name: '.env', content: r.data.env_content || '', removable: false },
				...r.data.extra_files.map((f) => ({ name: f.name, content: f.content, removable: true }))
			];
		}
		loading = false;
	}

	function validateYaml(c: string): string {
		if (!c.trim()) return $t('stacks.yamlEmpty');
		if (c.includes('\t')) return $t('stacks.yamlTabs');
		try { YAML.parse(c); return ''; } catch (e) { return (e as Error).message; }
	}
	function onInput() { if (activeTab === 0) yamlError = validateYaml(tabs[0].content); }
	function addFile() {
		const f = prompt('Filename:');
		if (!f?.trim()) return;
		if (tabs.some((t) => t.name === f.trim())) { toasts.error($t('stacks.alreadyExists')); return; }
		tabs = [...tabs, { name: f.trim(), content: '', removable: true }];
		activeTab = tabs.length - 1;
	}
	function removeTab(idx: number) {
		tabs = tabs.filter((_, i) => i !== idx);
		if (activeTab >= tabs.length) activeTab = tabs.length - 1;
	}

	async function save() {
		const err = validateYaml(tabs[0].content);
		if (err) { yamlError = err; activeTab = 0; toasts.error($t('stacks.yamlError')); return; }
		const extra: StackFile[] = tabs.slice(2).filter((t) => t.content.trim()).map((t) => ({ name: t.name, content: t.content }));
		saving = true;
		const r = await api.put<string>(`/env/${$selectedEnv}/stacks/${stackName}`, {
			compose_content: tabs[0].content, env_content: tabs[1].content.trim() || null,
			extra_files: extra.length > 0 ? extra : null
		});
		saving = false;
		if (r.success) toasts.success($t('common.save')); else toasts.error(r.error || $t('common.error'));
	}

	async function recreateContainer(id: string) {
		const c = detail?.containers.find(x => x.id === id);
		const name = c?.name || id.substring(0, 12);
		const image = c?.image || '?';

		recreateModal = {
			name, image, output: '', done: false,
			steps: [
				{ text: $t('containers.pullImage'), status: 'running' },
				{ text: $t('containers.stopRemove'), status: 'pending' },
				{ text: $t('containers.createStart'), status: 'pending' },
			]
		};

		setTimeout(() => { if (recreateModal && !recreateModal.done) { recreateModal.steps[0].status = 'done'; recreateModal.steps[1].status = 'running'; recreateModal = { ...recreateModal }; } }, 3000);
		setTimeout(() => { if (recreateModal && !recreateModal.done) { recreateModal.steps[1].status = 'done'; recreateModal.steps[2].status = 'running'; recreateModal = { ...recreateModal }; } }, 6000);

		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${id}/recreate`, {});

		if (r.success) {
			recreateModal.steps.forEach(s => { s.status = 'done'; });
			recreateModal.output = r.data || '';
			recreateModal.done = true;
			recreateModal = { ...recreateModal };
			setTimeout(load, 1500);
		} else {
			const failIdx = recreateModal.steps.findIndex(s => s.status === 'running');
			if (failIdx >= 0) recreateModal.steps[failIdx].status = 'error';
			recreateModal.output = r.error || $t('common.error');
			recreateModal.done = true;
			recreateModal = { ...recreateModal };
		}
	}

	async function deploy() {
		output = ''; toasts.success($t('stacks.stackStarting'));
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${stackName}/deploy`, {});
		if (r.success) { output = r.data || ''; toasts.success($t('stacks.started')); setTimeout(load, 2000); }
		else { output = r.error || ''; toasts.error($t('common.error')); }
	}
	async function stop() {
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${stackName}/stop`, {});
		if (r.success) { output = r.data || ''; toasts.success($t('containers.stop')); setTimeout(load, 1500); }
		else { output = r.error || ''; toasts.error($t('common.error')); }
	}

	async function containerAction(id: string, act: string) {
		if (act === 'remove') {
			confirm = { message: $t('containers.confirmDelete'), action: async () => { confirm = null; await doContainerAction(id, act); } };
			return;
		}
		await doContainerAction(id, act);
	}

	async function doContainerAction(id: string, act: string) {
		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${id}/action`, { action: act });
		if (r.success) { toasts.success($t('containers.successAction', { action: act })); setTimeout(load, 500); }
		else toasts.error(r.error || $t('common.error'));
	}

	function remove() {
		confirm = { message: $t('stacks.confirmDelete', { name: stackName }), action: async () => {
			confirm = null;
			await api.post<string>(`/env/${$selectedEnv}/stacks/${stackName}/stop`, {});
			const r = await api.del<string>(`/env/${$selectedEnv}/stacks/${stackName}`);
			if (r.success) { toasts.success($t('common.delete')); goto('/stacks'); }
			else toasts.error(r.error || $t('common.error'));
		}};
	}
</script>

<svelte:head><title>DockPit — Stack: {stackName}</title></svelte:head>

{#if loading}
	<div class="flex justify-center py-16"><div class="w-6 h-6 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
{:else if detail}
	<!-- Header -->
	<div class="flex items-center justify-between mb-5 flex-wrap gap-3">
		<div class="flex items-center gap-3">
			<a href="/stacks" class="w-8 h-8 flex items-center justify-center rounded-md border border-theme text-secondary hover:text-primary hover:border-light transition">
				<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>
			</a>
			<div>
				<h2 class="text-lg font-semibold text-primary">{stackName}</h2>
				<div class="flex items-center gap-2 mt-0.5">
					<Badge status={detail.status} />
					<span class="text-xs text-muted">{detail.running_services}/{detail.services_count} {$t('stacks.services')}</span>
					<span class="text-xs text-muted font-mono">· {detail.path}</span>
				</div>
			</div>
		</div>
		<div class="flex items-center gap-2 flex-wrap">
			{#if detail.status !== 'running'}
				<Button variant="success" size="sm" onclick={deploy}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>{$t('stacks.deploy')}</Button>
			{:else}
				<Button variant="warning" size="sm" onclick={stop}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>{$t('containers.stop')}</Button>
			{/if}
			<Button variant="secondary" size="sm" onclick={() => showEditor = !showEditor}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
				{showEditor ? $t('stacks.hideEditor') : $t('stacks.editor')}
			</Button>
			<Button variant="danger" size="sm" onclick={remove}>{$t('common.delete')}</Button>
		</div>
	</div>

	<!-- Containers -->
	<div class="bg-card border border-theme rounded-lg overflow-hidden mb-4">
		<div class="px-4 py-3 border-b border-theme flex items-center justify-between">
			<h3 class="text-sm font-semibold text-primary">{$t('containers.title')} ({detail.containers.length})</h3>
			<button onclick={load} class="text-xs text-muted hover:text-primary transition" title={$t('common.refresh')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</button>
		</div>
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.name')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">Image</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden xl:table-cell">IP</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden lg:table-cell">Ports</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden xl:table-cell">{$t('users.created')}</th>
					<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each detail.containers as c}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
							<td class="px-4 py-2.5">
								<div class="text-sm font-medium text-primary">{c.name}</div>
								<div class="text-[10px] font-mono text-muted">{c.id.substring(0, 12)}</div>
							</td>
							<td class="px-4 py-2.5 hidden md:table-cell">
								<div class="flex items-center gap-1.5">
									{#if isChecking(c.id)}
										<div class="w-3 h-3 border border-theme border-t-[var(--accent)] rounded-full animate-spin shrink-0"></div>
									{:else if getStatus(c.id) === 'up-to-date'}
										<span class="w-4 h-4 rounded-full bg-[var(--green)] flex items-center justify-center shrink-0" title={$t('containers.imageUpToDate')}>
											<svg class="w-2.5 h-2.5 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg></span>
									{:else if getStatus(c.id) === 'outdated'}
										<span class="w-4 h-4 rounded-full bg-[var(--red)] flex items-center justify-center shrink-0 cursor-pointer" title={$t('containers.updateClick')} onclick={() => recreateContainer(c.id)}>
											<svg class="w-2.5 h-2.5 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></span>
									{:else}
										<span class="w-4 h-4 rounded-full bg-3 border border-theme shrink-0"></span>
									{/if}
									<span class="text-xs text-secondary max-w-[140px] truncate">{c.image}</span>
								</div>
							</td>
							<td class="px-4 py-2.5"><Badge status={c.state} health={extractHealth(c.status)} /></td>
							<td class="px-4 py-2.5 text-[11px] font-mono text-secondary hidden xl:table-cell">{c.ip_address || '—'}</td>
							<td class="px-4 py-2.5 text-[11px] font-mono text-secondary hidden lg:table-cell">{formatPorts(c.ports)}</td>
							<td class="px-4 py-2.5 text-[11px] text-secondary hidden xl:table-cell">{formatDate(c.created)}</td>
							<td class="px-4 py-2.5">
								<div class="flex gap-1">
									{#if c.state !== 'running'}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" onclick={() => containerAction(c.id, 'start')} title={$t('containers.start')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></button>
									{:else}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" onclick={() => containerAction(c.id, 'stop')} title={$t('containers.stop')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></button>
									{/if}
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" onclick={() => containerAction(c.id, 'restart')} title={$t('containers.restart')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/></svg></button>
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:bg-purple-light hover:border-[var(--purple)] transition" onclick={() => recreateContainer(c.id)} title={$t('containers.recreate')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg></button>
									<a href="/containers/{c.id}/logs?stack={stackName}" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition no-underline" title={$t('containers.logs')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg></a>
									<a href="/containers/{c.id}/terminal?stack={stackName}" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--green)] hover:border-[var(--green)] transition no-underline" title={$t('containers.terminal')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg></a>
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="7" class="text-center py-8 text-sm text-muted">{$t('stacks.notStarted')}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
	</div>

	<!-- Editor (toggle) -->
	{#if showEditor}
		<div class="bg-card border border-theme rounded-lg overflow-hidden mb-4">
			<div class="px-4 py-3 border-b border-theme flex items-center justify-between">
				<h3 class="text-sm font-semibold text-primary">{$t('stacks.files')}</h3>
				<Button variant="primary" size="sm" onclick={save} loading={saving}>{$t('common.save')}</Button>
			</div>
			<div class="flex items-center border-b border-theme overflow-x-auto bg-1">
				{#each tabs as tab, i}
					<button class="flex items-center gap-1 px-3 py-2 text-[11px] font-medium whitespace-nowrap border-b-2 transition
						{activeTab === i ? 'border-[var(--accent)] text-accent' : 'border-transparent text-secondary hover:text-primary'}"
						onclick={() => activeTab = i}>
						{tab.name}
						{#if tab.removable}
							<span class="ml-1 w-3.5 h-3.5 flex items-center justify-center rounded hover:bg-red-light hover:text-red text-muted cursor-pointer"
								onclick={(e: MouseEvent) => { e.stopPropagation(); removeTab(i); }} role="button" tabindex="-1">
								<svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
							</span>
						{/if}
					</button>
				{/each}
				<button onclick={addFile} class="px-2 py-2 text-[11px] text-muted hover:text-accent transition" title={$t('stacks.addFile')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				</button>
			</div>
			{#if activeTab === 0 && yamlError}
				<div class="px-3 py-1.5 bg-red-light text-red text-[11px] border-b border-theme">YAML: {yamlError}</div>
			{/if}
			<textarea bind:value={tabs[activeTab].content} oninput={onInput} spellcheck={false}
				class="w-full h-[400px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border-none"></textarea>
			<div class="px-3 py-1.5 border-t border-theme flex items-center justify-between text-[10px] text-muted">
				<span>{tabs[activeTab].name}</span><span>{tabs[activeTab].content.split('\n').length} {$t('stacks.lines')}</span>
			</div>
		</div>
	{/if}

	<!-- Command output -->
	{#if output}
		<div class="bg-card border border-theme rounded-lg overflow-hidden">
			<div class="px-4 py-2.5 border-b border-theme flex items-center justify-between">
				<span class="text-xs font-semibold text-primary">{$t('stacks.output')}</span>
				<button class="text-[11px] text-muted hover:text-primary transition" onclick={() => output = ''}>{$t('common.close')}</button>
			</div>
			<div class="bg-0 p-3 font-mono text-[11px] leading-relaxed max-h-[250px] overflow-y-auto whitespace-pre-wrap break-all text-secondary">{output}</div>
		</div>
	{/if}
{/if}

{#if recreateModal}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[1000] p-4"
		onclick={(e) => { if (e.target === e.currentTarget && recreateModal?.done) recreateModal = null; }}>
		<div class="bg-card border border-theme rounded-xl w-full max-w-xl shadow-2xl flex flex-col max-h-[85vh]">
			<div class="flex items-center justify-between px-6 py-4 border-b border-theme shrink-0">
				<div>
					<h3 class="text-base font-semibold text-primary">{$t('containers.recreate')}</h3>
					<p class="text-xs text-secondary mt-0.5">{recreateModal.name} · {recreateModal.image}</p>
				</div>
				{#if recreateModal.done}
					<button class="w-8 h-8 flex items-center justify-center rounded-md border border-theme text-secondary hover:text-primary hover:border-light transition" onclick={() => recreateModal = null}>
						<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					</button>
				{/if}
			</div>
			<div class="px-6 py-5 space-y-4 overflow-y-auto">
				{#each recreateModal.steps as step, i}
					<div class="flex items-start gap-3">
						<div class="w-6 h-6 shrink-0 flex items-center justify-center mt-0.5">
							{#if step.status === 'running'}
								<div class="w-5 h-5 border-2 border-[var(--accent)]/30 border-t-[var(--accent)] rounded-full animate-spin"></div>
							{:else if step.status === 'done'}
								<div class="w-5 h-5 rounded-full bg-[var(--green)] flex items-center justify-center">
									<svg class="w-3 h-3 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg></div>
							{:else if step.status === 'error'}
								<div class="w-5 h-5 rounded-full bg-[var(--red)] flex items-center justify-center">
									<svg class="w-3 h-3 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></div>
							{:else}
								<div class="w-5 h-5 rounded-full border-2 border-theme"></div>
							{/if}
						</div>
						<div class="flex-1 min-w-0">
							<p class="text-sm {step.status === 'running' ? 'text-primary font-medium' : step.status === 'done' ? 'text-secondary' : step.status === 'error' ? 'text-red' : 'text-muted'}">{step.text}</p>
							{#if step.detail}<p class="text-[11px] text-muted mt-1 font-mono break-all">{step.detail}</p>{/if}
						</div>
					</div>
					{#if i < recreateModal.steps.length - 1}<div class="ml-3 w-px h-2 bg-3"></div>{/if}
				{/each}
			</div>
			{#if recreateModal.output && recreateModal.done}
				<div class="border-t border-theme">
					<details class="group">
						<summary class="px-6 py-3 text-xs text-secondary cursor-pointer hover:text-primary transition flex items-center gap-2">
							<svg class="w-3 h-3 transition group-open:rotate-90" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
							{$t('containers.fullOutput')}
						</summary>
						<div class="px-6 pb-4">
							<div class="bg-0 border border-theme rounded-lg p-4 font-mono text-[11px] leading-[1.8] text-secondary max-h-[200px] overflow-y-auto whitespace-pre-wrap break-words">{recreateModal.output}</div>
						</div>
					</details>
				</div>
			{/if}
			{#if recreateModal.done}
				<div class="px-6 py-4 border-t border-theme flex justify-end shrink-0">
					<Button variant="primary" onclick={() => recreateModal = null}>{$t('common.close')}</Button>
				</div>
			{/if}
		</div>
	</div>
{/if}

{#if confirm}
	<ConfirmDialog message={confirm.message} onconfirm={confirm.action} oncancel={() => confirm = null} />
{/if}
