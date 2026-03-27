<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { selectedEnv } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import YAML from 'yaml';
	import { canManageDocker, canEditContainers } from '$lib/stores/auth';
	import type { StackInfo, StackFile } from '$lib/api/types';

	let stacks = $state<StackInfo[]>([]);
	let loading = $state(true);
	let search = $state('');
	let showCreate = $state(false);
	let page = $state(1);
	let perPage = $state(10);
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);

	// Create modal state
	let newName = $state('');
	let createTab = $state(0);
	let saving = $state(false);
	let yamlError = $state('');

	interface Tab { name: string; content: string; removable: boolean; }
	let tabs = $state<Tab[]>([]);

	function resetCreate() {
		newName = ''; createTab = 0; yamlError = '';
		tabs = [
			{ name: 'docker-compose.yml', content: `services:\n  app:\n    image: nginx:latest\n    ports:\n      - "8080:80"\n    restart: unless-stopped\n`, removable: false },
			{ name: '.env', content: '', removable: false }
		];
	}

	const filtered = $derived(stacks.filter((s) => s.name.toLowerCase().includes(search.toLowerCase())));
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));

	// Stats
	const runningCount = $derived(stacks.filter(s => s.status === 'running').length);
	const stoppedCount = $derived(stacks.filter(s => s.status === 'stopped').length);

	onMount(() => { resetCreate(); load(); });
	$effect(() => { $selectedEnv; load(); });
	$effect(() => { search; page = 1; }); // Reset page on search

	async function load() {
		if (!$selectedEnv) return;
		loading = true;
		const r = await api.get<StackInfo[]>(`/env/${$selectedEnv}/stacks`);
		if (r.success) stacks = r.data || [];
		loading = false;
	}

	function validateYaml(c: string): string {
		if (!c.trim()) return $t('stacks.yamlEmpty');
		if (c.includes('\t')) return $t('stacks.yamlTabs');
		try { YAML.parse(c); return ''; } catch (e) { return (e as Error).message; }
	}

	function onInput() { if (createTab === 0) yamlError = validateYaml(tabs[0].content); }

	function addFile() {
		const f = prompt('Filename (e.g. prometheus.yml):');
		if (!f?.trim()) return;
		if (tabs.some((t) => t.name === f.trim())) { toasts.error($t('stacks.alreadyExists')); return; }
		tabs = [...tabs, { name: f.trim(), content: '', removable: true }];
		createTab = tabs.length - 1;
	}

	function removeTab(idx: number) {
		tabs = tabs.filter((_, i) => i !== idx);
		if (createTab >= tabs.length) createTab = tabs.length - 1;
	}

	async function create() {
		if (!newName.trim()) { toasts.error($t('stacks.stackNameRequired')); return; }
		const err = validateYaml(tabs[0].content);
		if (err) { yamlError = err; createTab = 0; toasts.error($t('stacks.yamlError')); return; }
		const extra: StackFile[] = tabs.slice(2).filter((t) => t.content.trim()).map((t) => ({ name: t.name, content: t.content }));
		saving = true;
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks`, {
			name: newName.trim(), compose_content: tabs[0].content,
			env_content: tabs[1].content.trim() || null,
			extra_files: extra.length > 0 ? extra : null
		});
		saving = false;
		if (r.success) { showCreate = false; toasts.success($t('stacks.created', { name: newName })); resetCreate(); load(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function deploy(name: string) {
		toasts.success($t('stacks.stackStarting'));
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${name}/deploy`, {});
		if (r.success) { toasts.success($t('stacks.started')); setTimeout(load, 1500); } else toasts.error(r.error || $t('common.error'));
	}

	async function stop(name: string) {
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${name}/stop`, {});
		if (r.success) { toasts.success($t('containers.stop')); setTimeout(load, 1000); } else toasts.error(r.error || $t('common.error'));
	}

	function remove(name: string) {
		confirmDlg = { message: $t('stacks.confirmDelete', { name }), action: async () => {
			confirmDlg = null;
			await api.post<string>(`/env/${$selectedEnv}/stacks/${name}/stop`, {});
			const r = await api.del<string>(`/env/${$selectedEnv}/stacks/${name}`);
			if (r.success) { toasts.success($t('common.delete')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — {$t('stacks.title')}</title></svelte:head>

<!-- Summary -->
{#if !loading}
	{@const totalServices = stacks.reduce((a, s) => a + s.services_count, 0)}
	{@const runningServices = stacks.reduce((a, s) => a + s.running_services, 0)}
	<div class="grid grid-cols-2 md:grid-cols-5 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{stacks.length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.title')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{stacks.filter(s => s.status === 'running').length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.active')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-yellow">{stacks.filter(s => s.status === 'partial').length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.partial')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-red">{stacks.filter(s => s.status === 'stopped').length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.stopped')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{runningServices}<span class="text-sm font-normal text-muted">/{totalServices}</span></div>
			<div class="text-[11px] text-secondary">{$t('stacks.services')}</div>
		</div>
	</div>
{/if}

<!-- Table -->
<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<h3 class="text-sm font-semibold text-primary">{$t('stacks.title')}</h3>
		<div class="flex items-center gap-2">
			<input bind:value={search} placeholder={$t('common.search')}
				class="bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs w-44 focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200" />
			<Button variant="primary" size="sm" onclick={() => { resetCreate(); showCreate = true; }}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				{$t('stacks.newStack')}
			</Button>
			<button onclick={load} title={$t('common.refresh')}
				class="inline-flex items-center justify-center w-8 h-8 border border-theme text-secondary hover:text-primary hover:border-light rounded-md transition">
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</button>
		</div>
	</div>

	{#if loading}
		<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.name')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.status')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('stacks.services')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('stacks.path')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as s}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
							<td class="px-4 py-3">
								<a href="/stacks/{s.name}" class="text-sm font-medium text-accent hover:text-accent-hover transition">{s.name}</a>
							</td>
							<td class="px-4 py-3"><Badge status={s.status} /></td>
							<td class="px-4 py-3">
								<span class="text-xs">
									<span class="text-green font-medium">{s.running_services}</span>
									<span class="text-muted"> / {s.services_count}</span>
								</span>
							</td>
							<td class="px-4 py-3 text-[11px] font-mono text-muted max-w-[200px] truncate hidden md:table-cell">{s.path}</td>
							<td class="px-4 py-3">
								<div class="flex gap-1">
									{#if s.status !== 'running'}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--green)] hover:border-[var(--green)] transition" onclick={() => deploy(s.name)} title={$t('stacks.deploy')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></button>
									{:else}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--yellow)] hover:border-[var(--yellow)] transition" onclick={() => stop(s.name)} title={$t('containers.stop')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></button>
									{/if}
									<a href="/stacks/{s.name}" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition no-underline" title={$t('common.edit')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg></a>
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--red)] hover:border-[var(--red)] transition" onclick={() => remove(s.name)} title={$t('common.delete')}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg></button>
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="5" class="text-center py-10 text-sm text-muted">
							{search ? $t('stacks.noStacks') : $t('stacks.noStacksEmpty')}
						</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
		<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
	{/if}
</div>

<!-- Create Stack Modal -->
{#if showCreate}
	<Modal title={$t('stacks.newStack')} onclose={() => showCreate = false}>
		<div class="mb-4">
			<TextInput bind:value={newName} label={$t('stacks.stackName')} placeholder="e.g. argus, monitoring" id="sn" />
			<p class="text-[10px] text-muted mt-1">{$t('stacks.stackNameHint')}</p>
		</div>

		<div class="flex items-center border border-theme rounded-t-lg overflow-x-auto bg-1">
			{#each tabs as tab, i}
				<button class="flex items-center gap-1 px-3 py-2 text-[11px] font-medium whitespace-nowrap border-b-2 transition
					{createTab === i ? 'border-[var(--accent)] text-accent' : 'border-transparent text-secondary hover:text-primary'}"
					onclick={() => createTab = i}>
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

		{#if createTab === 0 && yamlError}
			<div class="px-3 py-1.5 bg-red-light text-red text-[11px] border-x border-theme">YAML: {yamlError}</div>
		{/if}

		<textarea bind:value={tabs[createTab].content} oninput={onInput} spellcheck={false}
			class="w-full h-[300px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border border-t-0 border-theme rounded-b-lg"
			placeholder={createTab === 0 ? 'services:\n  app:\n    image: ...' : createTab === 1 ? 'KEY=value' : ''}></textarea>

		<div class="flex items-center justify-between mt-1 text-[10px] text-muted px-1">
			<span>{tabs[createTab].name}</span>
			<span>{tabs[createTab].content.split('\n').length} {$t('stacks.lines')}</span>
		</div>

		<div class="flex justify-end gap-2 mt-4">
			<Button variant="secondary" size="sm" onclick={() => showCreate = false}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={create} loading={saving}>{$t('stacks.createStack')}</Button>
		</div>
	</Modal>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
