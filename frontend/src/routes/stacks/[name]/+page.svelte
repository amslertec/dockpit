<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Badge from '$lib/components/ui/Badge.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import YAML from 'yaml';
	import { formatPorts, formatDate, formatDateTime, extractHealth } from '$lib/utils/format';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import { canSeePage, canDoAction } from '$lib/stores/auth';
	import type { StackDetail, StackFile, ImageUpdateCheck } from '$lib/api/types';

	const stackName = $derived($page.params.name);

	$effect(() => {
		if (!$canSeePage('page.stacks')) goto('/profile');
	});

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

	let skipNextUpdateCheck: string | false = false; // false or image name to carry over

	async function load() {
		if (!$selectedEnv || !stackName) return;
		loading = true;
		const r = await api.get<StackDetail>(`/env/${$selectedEnv}/stacks/${stackName}`);
		if (r.success && r.data) {
			detail = r.data;
			if (skipNextUpdateCheck) {
				// Carry over up-to-date status to new container IDs with same image
				const img = skipNextUpdateCheck;
				skipNextUpdateCheck = false;
				const m = new Map(updateStatus);
				for (const c of r.data.containers) {
					if (c.image === img) m.set(c.id, 'up-to-date');
				}
				updateStatus = m;
			} else {
				checkUpdatesInBackground();
			}
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
			// Mark all containers of this image as up-to-date (new container gets new ID)
			const img = detail?.containers.find(x => x.id === id)?.image;
			const m = new Map(updateStatus);
			if (img && detail) {
				for (const c of detail.containers) {
					if (c.image === img) m.set(c.id, 'up-to-date');
				}
			}
			m.set(id, 'up-to-date');
			updateStatus = m;
			skipNextUpdateCheck = img || true;
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

	// Rollback
	let rollbackModal = $state<{ id: string; name: string } | null>(null);
	let snapshots = $state<{id: number; image: string; created_at: string}[]>([]);
	let rollingBack = $state(false);

	async function loadSnapshots(name: string) {
		const r = await api.get<{id: number; image: string; created_at: string}[]>(`/snapshots/${encodeURIComponent(name)}`);
		if (r.success && r.data) snapshots = r.data; else snapshots = [];
	}
	async function doRollback(snapshotId: number) {
		if (!rollbackModal) return;
		const cId = rollbackModal.id;
		const cName = rollbackModal.name;
		const snap = snapshots.find(s => s.id === snapshotId);
		rollbackModal = null; snapshots = [];
		recreateModal = {
			name: cName, image: snap?.image || '?', output: '', done: false,
			steps: [
				{ text: $t('containers.rollbackPull'), status: 'running' },
				{ text: $t('containers.stopRemove'), status: 'pending' },
				{ text: $t('containers.rollbackRestore'), status: 'pending' },
			]
		};
		setTimeout(() => { if (recreateModal && !recreateModal.done) { recreateModal.steps[0].status = 'done'; recreateModal.steps[1].status = 'running'; recreateModal = { ...recreateModal }; } }, 3000);
		setTimeout(() => { if (recreateModal && !recreateModal.done) { recreateModal.steps[1].status = 'done'; recreateModal.steps[2].status = 'running'; recreateModal = { ...recreateModal }; } }, 6000);
		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${cId}/rollback`, { snapshot_id: snapshotId });
		if (r.success) {
			recreateModal!.steps.forEach(s => { s.status = 'done'; });
			recreateModal!.output = r.data || '';
			recreateModal!.done = true;
			recreateModal = { ...recreateModal! };
			setTimeout(load, 1500);
		} else {
			const failIdx = recreateModal!.steps.findIndex(s => s.status === 'running');
			if (failIdx >= 0) recreateModal!.steps[failIdx].status = 'error';
			recreateModal!.output = r.error || $t('common.error');
			recreateModal!.done = true;
			recreateModal = { ...recreateModal! };
		}
	}

	async function deleteSnapshot(snapshotId: number) {
		const r = await api.del<string>(`/snapshots/delete/${snapshotId}`);
		if (r.success) { snapshots = snapshots.filter(s => s.id !== snapshotId); toasts.success($t('containers.snapshotDeleted')); }
		else toasts.error(r.error || $t('common.error'));
	}

	// Migrate container
	let migrateModal = $state<{ id: string; name: string; image: string } | null>(null);
	let migrateTarget = $state('');
	let migrateStopSource = $state(true);
	let migrating = $state(false);
	let migrateDropdown = $state(false);
	const migrateTargets = $derived($environments.filter(e => e.id !== $selectedEnv).map(e => ({ value: e.id, label: e.name })));

	async function migrateContainer() {
		if (!migrateModal || !migrateTarget) return;
		migrating = true;
		const r = await api.post<string>(`/env/${$selectedEnv}/containers/${migrateModal.id}/migrate`, { target_env_id: migrateTarget, stop_source: migrateStopSource });
		migrating = false;
		if (r.success) { toasts.success(r.data || $t('containers.migrated')); migrateModal = null; migrateTarget = ''; load(); }
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
			<a href="/stacks" class="w-8 h-8 flex items-center justify-center rounded-md border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition">
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
			{#if $canDoAction('action.stack_deploy_stop')}
				{#if detail.status !== 'running'}
					<Button variant="success" size="sm" onclick={deploy} title={$t('stacks.deploy')}>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></Button>
				{:else}
					<Button variant="warning" size="sm" onclick={stop} title={$t('containers.stop')}>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></Button>
				{/if}
			{/if}
			{#if $canDoAction('action.stack_edit')}
				<Button variant="purple" size="sm" onclick={() => showEditor = !showEditor} title={showEditor ? $t('stacks.hideEditor') : $t('stacks.editor')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
				</Button>
			{/if}
			{#if $canDoAction('action.stack_create_delete')}
				<Button variant="danger" size="sm" onclick={remove} title={$t('common.delete')}>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
				</Button>
			{/if}
		</div>
	</div>

	<!-- Containers -->
	<div class="bg-card border border-theme rounded-lg overflow-hidden mb-4">
		<div class="px-4 py-3 border-b border-theme flex items-center justify-between">
			<h3 class="text-sm font-semibold text-primary">{$t('containers.title')} ({detail.containers.length})</h3>
			<Button variant="success" size="sm" onclick={load} title={$t('common.refresh')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</Button>
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
								<a href="/containers/{c.id}" class="text-sm font-medium text-primary hover:text-[var(--accent)] transition no-underline">{c.name}</a>
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
									{#if $canDoAction('action.container_start_stop')}
										{#if c.state !== 'running'}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--green)] hover:border-[var(--green)]/40 hover:bg-[var(--green)]/8 transition" onclick={() => containerAction(c.id, 'start')} title={$t('containers.start')}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></button>
										{:else}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => containerAction(c.id, 'stop')} title={$t('containers.stop')}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></button>
										{/if}
									{/if}
									{#if $canDoAction('action.container_restart')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition" onclick={() => containerAction(c.id, 'restart')} title={$t('containers.restart')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/></svg></button>
									{/if}
									{#if $canDoAction('action.container_recreate')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition" onclick={() => recreateContainer(c.id)} title={$t('containers.recreate')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg></button>
									{/if}
									{#if $canDoAction('action.container_logs')}
										<a href="/containers/{c.id}/logs?stack={stackName}" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition no-underline" title={$t('containers.logs')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><path d="M14 2v6h6"/></svg></a>
									{/if}
									{#if $canDoAction('action.container_terminal')}
										<a href="/containers/{c.id}/terminal?stack={stackName}" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--green)] hover:border-[var(--green)]/40 hover:bg-[var(--green)]/8 transition no-underline" title={$t('containers.terminal')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg></a>
									{/if}
									{#if $canDoAction('action.container_rollback')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--yellow)] hover:border-[var(--yellow)]/40 hover:bg-[var(--yellow)]/8 transition" onclick={async () => { rollbackModal = { id: c.id, name: c.name }; await loadSnapshots(c.name); }} title={$t('containers.rollback')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12a9 9 0 109-9"/><polyline points="3 3 3 9 9 9"/><path d="M3 9l3-3"/></svg></button>
									{/if}
									{#if $canDoAction('action.container_migrate')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-[var(--accent)] hover:border-[var(--accent)] transition" onclick={() => migrateModal = { id: c.id, name: c.name, image: c.image }} title={$t('containers.migrate')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M12 5l7 7-7 7"/></svg></button>
									{/if}
									{#if $canDoAction('action.container_delete')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => containerAction(c.id, 'remove')} title={$t('common.delete')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg></button>
									{/if}
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

{#if rollbackModal}
	<Modal title={$t('containers.rollbackTitle')} onclose={() => { rollbackModal = null; snapshots = []; }}>
		<div class="space-y-3">
			<p class="text-sm text-secondary">{$t('containers.rollbackDesc')}</p>
			<p class="text-sm font-medium text-primary">{rollbackModal.name}</p>
			{#if snapshots.length === 0}
				<div class="text-center py-6 text-sm text-muted">{$t('containers.noSnapshots')}</div>
			{:else}
				<div class="space-y-2 max-h-[300px] overflow-y-auto">
					{#each snapshots as snap}
						<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3">
							<div class="flex items-center justify-between gap-3">
								<div class="min-w-0 flex-1">
									<div class="text-xs font-mono text-primary truncate">{snap.image}</div>
									<div class="text-[10px] text-muted mt-0.5">{formatDateTime(snap.created_at)}</div>
								</div>
								<div class="flex items-center gap-1.5 shrink-0">
									<Button variant="secondary" size="sm" onclick={() => doRollback(snap.id)} loading={rollingBack}>
										{$t('containers.rollbackTo')}
									</Button>
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-muted hover:text-[var(--red)] hover:border-[var(--red)] transition" onclick={() => deleteSnapshot(snap.id)} title={$t('common.delete')}>
										<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
									</button>
								</div>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</Modal>
{/if}

{#if migrateModal}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1000] p-0 sm:p-4" onclick={(e) => { if (e.target === e.currentTarget) { migrateModal = null; migrateDropdown = false; } }}>
		<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-lg shadow-[var(--shadow-lg)]" style="overflow:visible; background:var(--glass-bg); backdrop-filter:blur(20px) saturate(150%); -webkit-backdrop-filter:blur(20px) saturate(150%)">
			<div class="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
				<h3 class="text-[15px] font-semibold text-[var(--text)]">{$t('containers.migrateTitle')}</h3>
				<button class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all" onclick={() => { migrateModal = null; migrateDropdown = false; }}>
					<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>
			<div class="p-5 space-y-4">
				<div>
					<p class="text-sm text-secondary mb-1">{$t('containers.migrateContainer')}</p>
					<p class="text-sm font-medium text-primary">{migrateModal.name}</p>
					<p class="text-xs text-muted mt-0.5">{migrateModal.image}</p>
				</div>
				<div>
					<label class="block text-xs font-medium text-secondary mb-1">{$t('containers.migrateTarget')}</label>
					<div class="relative">
						<button
							class="w-full flex items-center gap-2.5 h-9 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-3)] text-[var(--text)] hover:border-[var(--border-light)] hover:shadow-[var(--shadow-sm)] transition-all cursor-pointer"
							onclick={(e) => { e.stopPropagation(); migrateDropdown = !migrateDropdown; }}
						>
							{#if migrateTarget}
								{@const env = $environments.find(e => e.id === migrateTarget)}
								<span class="w-2 h-2 rounded-full shrink-0 {env?.status === 'online' || env?.is_local ? 'bg-[var(--green)] shadow-[var(--shadow-glow-green)]' : 'bg-[var(--red)]'}"></span>
								<span class="truncate font-medium">{env?.name}</span>
							{:else}
								<span class="text-muted">{$t('containers.selectTarget')}</span>
							{/if}
							<svg class="w-3.5 h-3.5 text-muted shrink-0 ml-auto transition-transform duration-200 {migrateDropdown ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
						</button>
						{#if migrateDropdown}
							<div class="absolute left-0 right-0 mt-1.5 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] z-[1100] py-1.5">
								<div class="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)]">{$t('containers.migrateTarget')}</div>
								{#each migrateTargets as t}
									{@const env = $environments.find(e => e.id === t.value)}
									<button
										class="w-full flex items-center gap-3 px-3 py-2.5 text-xs text-left transition-all duration-150
										{t.value === migrateTarget ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text)]'}"
										onclick={() => { migrateTarget = t.value; migrateDropdown = false; }}
									>
										<span class="w-2 h-2 rounded-full shrink-0 {env?.status === 'online' || env?.is_local ? 'bg-[var(--green)]' : 'bg-[var(--red)]'}"></span>
										<span class="truncate font-medium">{t.label}</span>
										{#if t.value === migrateTarget}
											<svg class="w-3.5 h-3.5 shrink-0 ml-auto text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
										{/if}
									</button>
								{/each}
							</div>
						{/if}
					</div>
				</div>
				<CustomCheckbox checked={migrateStopSource} onchange={(v) => migrateStopSource = v} label={$t('containers.migrateStopSource')} />
			</div>
			<div class="px-5 py-3 border-t border-[var(--border)] flex justify-end gap-2">
				<Button variant="danger" size="sm" onclick={() => { migrateModal = null; migrateDropdown = false; }}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" onclick={migrateContainer} loading={migrating} disabled={!migrateTarget}>{$t('containers.migrateStart')}</Button>
			</div>
		</div>
	</div>
{/if}

{#if confirm}
	<ConfirmDialog message={confirm.message} onconfirm={confirm.action} oncancel={() => confirm = null} />
{/if}
