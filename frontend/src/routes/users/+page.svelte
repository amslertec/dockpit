<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { canDoAction } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import Tabs from '$lib/components/ui/Tabs.svelte';
	import { formatDateTime } from '$lib/utils/format';

	$effect(() => {
		if (!$canDoAction('action.user_management')) goto('/profile');
	});

	let activeTab = $state(0);
	let loading = $state(true);

	// Users
	let users = $state<any[]>([]);
	let showCreate = $state(false);
	let newUser = $state('');
	let newPass = $state('');
	let newRole = $state('user');
	let creating = $state(false);
	let editUser = $state<any | null>(null);
	let editRole = $state('');
	let editPass = $state('');
	let editSaving = $state(false);
	let assignGroupsUser = $state<any | null>(null);
	let assignGroupIds = $state<number[]>([]);

	// Groups
	let groups = $state<any[]>([]);
	let showCreateGroup = $state(false);
	let newGroupName = $state('');
	let newGroupDesc = $state('');
	let newGroupColor = $state('#6c5ce7');
	let editGroup = $state<any | null>(null);
	let editPerms = $state<string[]>([]);
	let editGroupInfo = $state<any | null>(null);
	let editGroupName = $state('');
	let editGroupDesc = $state('');
	let editGroupColor = $state('#6c5ce7');
	let assignDropdownOpen = $state(false);

	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);


	const tabs = $derived([
		{ id: 0, label: $t('nav.users') },
		{ id: 1, label: $t('groups.title') },
	]);

	const roles = $derived([
		{ value: 'super_admin', label: 'Super Admin', desc: $t('users.roleSuperAdminDesc') },
		{ value: 'admin', label: 'Admin', desc: $t('users.roleAdminDesc') },
		{ value: 'editor', label: 'Editor', desc: $t('users.roleEditorDesc') },
		{ value: 'viewer', label: 'Viewer', desc: $t('users.roleViewerDesc') },
		{ value: 'user', label: $t('users.roleUser'), desc: $t('users.roleUserDesc') },
	]);

	// Grouped permissions: page + related actions
	const permSections = [
		{ page: 'page.dashboard', label: 'Dashboard', icon: '📊', actions: [] },
		{ page: 'page.containers', label: 'Containers', icon: '📦', actions: [
			{ key: 'action.container_start_stop', label: 'Start / Stop' },
			{ key: 'action.container_restart', label: 'Restart' },
			{ key: 'action.container_recreate', label: 'Recreate / Update' },
			{ key: 'action.container_delete', label: 'Delete' },
			{ key: 'action.container_logs', label: 'Logs' },
			{ key: 'action.container_terminal', label: 'Terminal' },
			{ key: 'action.container_inspect', label: 'Details' },
			{ key: 'action.container_migrate', label: 'Migrate' },
			{ key: 'action.container_rollback', label: 'Rollback' },
		]},
		{ page: 'page.stacks', label: 'Stacks', icon: '🗂️', actions: [
			{ key: 'action.stack_deploy_stop', label: 'Deploy / Stop' },
			{ key: 'action.stack_create_delete', label: 'Create / Delete' },
			{ key: 'action.stack_edit', label: 'Edit (Compose)' },
			{ key: 'action.stack_migrate', label: 'Migrate' },
		]},
		{ page: 'page.images', label: 'Images', icon: '🖼️', actions: [
			{ key: 'action.image_pull_delete', label: 'Pull / Delete / Prune' },
		]},
		{ page: 'page.volumes', label: 'Volumes', icon: '💾', actions: [
			{ key: 'action.volume_delete', label: 'Delete / Prune' },
		]},
		{ page: 'page.networks', label: 'Networks', icon: '🌐', actions: [
			{ key: 'action.network_delete', label: 'Delete / Prune' },
		]},
		{ page: 'page.monitoring', label: 'Monitoring', icon: '📈', actions: [] },
		{ page: 'page.health', label: 'Health Checks', icon: '❤️', actions: [] },
		{ page: 'page.events', label: 'Events', icon: '🕐', actions: [] },
		{ page: 'page.updates', label: 'Updates', icon: '⬇️', actions: [
			{ key: 'action.container_recreate', label: 'Check / Recreate' },
		]},
		{ page: 'page.vulnerabilities', label: 'Vulnerabilities', icon: '🛡️', actions: [
			{ key: 'action.vuln_scan', label: 'Scan' },
		]},
		{ page: 'page.audit', label: 'Audit Log', icon: '📋', actions: [] },
		{ page: 'page.host_terminal', label: 'Host Terminal', icon: '💻', actions: [
			{ key: 'action.host_terminal_connect', label: 'Connect' },
		]},
		{ page: 'page.environments', label: 'Environments', icon: '🖥️', actions: [
			{ key: 'action.env_edit', label: 'Edit / Delete' },
			{ key: 'action.env_connect', label: 'Connect (Agent)' },
			{ key: 'action.docker_login', label: 'Docker Registry Login' },
			{ key: 'action.scheduled_jobs', label: 'Scheduled Jobs' },
		]},
		{ page: 'page.settings', label: 'Settings', icon: '⚙️', actions: [
			{ key: 'action.settings_general', label: 'General Settings' },
			{ key: 'action.settings_updates', label: 'Update Monitor' },
			{ key: 'action.settings_webhooks', label: 'Webhooks' },
			{ key: 'action.settings_email', label: 'Email' },
			{ key: 'action.backup', label: 'Backup / Restore' },
			{ key: 'action.settings_alerts', label: 'Alert Rules' },
		]},
	];

	// System-wide permissions (not tied to a page)
	const systemPerms = [
		{ key: 'action.server_switch', label: 'Server Switch (Topbar)' },
		{ key: 'action.user_management', label: 'User Management' },
	];

	let expandedSections = $state<Set<string>>(new Set());

	function toggleSection(page: string) {
		const next = new Set(expandedSections);
		if (next.has(page)) next.delete(page); else next.add(page);
		expandedSections = next;
	}

	function togglePageWithSuggestions(page: string, actions: {key: string}[]) {
		if (editPerms.includes(page)) {
			// Removing page: also remove all its actions
			editPerms = editPerms.filter(p => p !== page && !actions.some(a => a.key === p));
		} else {
			// Adding page: only add the page permission, expand section for manual action selection
			editPerms = [...editPerms, page];
			if (actions.length > 0) expandedSections = new Set([...expandedSections, page]);
		}
	}

	function toggleAllSectionActions(page: string, actions: {key: string}[]) {
		const allSelected = actions.every(a => editPerms.includes(a.key));
		if (allSelected) {
			editPerms = editPerms.filter(p => !actions.some(a => a.key === p));
		} else {
			editPerms = [...new Set([...editPerms, ...actions.map(a => a.key)])];
		}
	}

	function selectAllPerms() {
		const all: string[] = [];
		for (const s of permSections) { all.push(s.page); for (const a of s.actions) all.push(a.key); }
		for (const p of systemPerms) all.push(p.key);
		const allSelected = all.every(p => editPerms.includes(p));
		editPerms = allSelected ? [] : [...new Set(all)];
	}

	function selectNonePerms() { editPerms = []; }

	// Translate default group descriptions
	const defaultGroupDescs: Record<string, string> = { 'DockPit': 'groups.descDockPit', 'Admin': 'groups.descAdmin', 'Editor': 'groups.descEditor', 'Viewer': 'groups.descViewer' };
	function groupDesc(g: any): string {
		const key = defaultGroupDescs[g.name];
		return key ? $t(key) : (g.description || '');
	}

	// Flat list for backward compat (used in toggleAllPerms)
	const allPagePerms = permSections.map(s => ({ key: s.page, label: s.label }));
	const allActionPerms = permSections.flatMap(s => s.actions).concat(systemPerms);

	const superAdminCount = $derived(users.filter(u => u.role === 'super_admin').length);

	function canDeleteUser(u: any): boolean {
		if (u.role === 'super_admin' && superAdminCount <= 1) return false;
		return true;
	}

	onMount(() => loadAll());

	async function loadAll() {
		loading = true;
		const [uR, gR] = await Promise.all([
			api.get<any[]>('/users'),
			api.get<any[]>('/groups'),
		]);
		if (uR.success && uR.data) users = uR.data;
		if (gR.success && gR.data) groups = gR.data;
		loading = false;
	}

	function roleBadge(role: string): string {
		const m: Record<string, string> = { super_admin: 'bg-purple-light text-purple', admin: 'bg-accent-light text-accent', editor: 'bg-yellow-light text-yellow', viewer: 'bg-3 text-secondary', user: 'bg-green-light text-green' };
		return m[role] || 'bg-3 text-secondary';
	}

	function roleLabel(role: string): string {
		return roles.find(r => r.value === role)?.label || role;
	}

	// User CRUD
	async function createUser(e: Event) {
		e.preventDefault();
		creating = true;
		const r = await api.post<string>('/users', { username: newUser, password: newPass, role: newRole });
		creating = false;
		if (r.success) { showCreate = false; newUser = ''; newPass = ''; newRole = 'user'; toasts.success($t('users.created')); loadAll(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function saveUser(e: Event) {
		e.preventDefault();
		if (!editUser) return;
		editSaving = true;
		const body: any = { role: editRole };
		if (editPass) body.password = editPass;
		const r = await api.put<string>(`/users/${editUser.id}`, body);
		editSaving = false;
		if (r.success) { editUser = null; editPass = ''; toasts.success($t('common.save')); loadAll(); }
		else toasts.error(r.error || $t('common.error'));
	}

	function deleteUser(id: string, username: string) {
		confirmDlg = { message: $t('users.deleteConfirm', { name: username }), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/users/${id}`);
			if (r.success) { toasts.success($t('common.delete')); loadAll(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	async function resetMfa(id: string) {
		const r = await api.post<string>(`/users/${id}/reset-mfa`, {});
		if (r.success) toasts.success('2FA reset'); else toasts.error(r.error || $t('common.error'));
	}

	// Group assignments
	function openAssignGroups(user: any) {
		assignGroupsUser = user;
		assignGroupIds = (user.groups || []).map((g: any) => g.id);
	}

	function toggleGroupAssign(gid: number) {
		if (assignGroupIds.includes(gid)) assignGroupIds = assignGroupIds.filter(id => id !== gid);
		else assignGroupIds = [...assignGroupIds, gid];
	}

	async function saveGroupAssignment() {
		if (!assignGroupsUser) return;
		await api.put<string>(`/users/${assignGroupsUser.id}/groups`, { group_ids: assignGroupIds });
		assignGroupsUser = null;
		toasts.success($t('groups.assignGroups'));
		loadAll();
	}

	// Group CRUD
	async function createGroup(e: Event) {
		e.preventDefault();
		const r = await api.post<string>('/groups', { name: newGroupName, description: newGroupDesc || null, color: newGroupColor });
		if (r.success) { showCreateGroup = false; newGroupName = ''; newGroupDesc = ''; newGroupColor = '#6c5ce7'; toasts.success($t('groups.create')); loadAll(); }
		else toasts.error(r.error || $t('common.error'));
	}

	function openEditGroupInfo(g: any) {
		editGroupInfo = g;
		editGroupName = g.name;
		editGroupDesc = g.description || '';
		editGroupColor = g.color || '#6c5ce7';
	}

	async function saveGroupInfo() {
		if (!editGroupInfo || !editGroupName.trim()) return;
		await api.put<string>(`/groups/${editGroupInfo.id}`, { name: editGroupName, description: editGroupDesc || null, color: editGroupColor });
		editGroupInfo = null;
		toasts.success($t('common.save'));
		loadAll();
	}

	function openEditGroup(group: any) {
		editGroup = group;
		editPerms = [...(group.permissions || [])];
	}

	function togglePerm(key: string) {
		if (editPerms.includes(key)) editPerms = editPerms.filter(p => p !== key);
		else editPerms = [...editPerms, key];
	}

	function toggleAllPerms(perms: {key: string}[]) {
		const allSelected = perms.every(p => editPerms.includes(p.key));
		if (allSelected) editPerms = editPerms.filter(p => !perms.some(pp => pp.key === p));
		else editPerms = [...new Set([...editPerms, ...perms.map(p => p.key)])];
	}

	async function saveGroupPerms() {
		if (!editGroup) return;
		await api.put<string>(`/groups/${editGroup.id}`, { permissions: editPerms });
		editGroup = null;
		toasts.success($t('common.save'));
		loadAll();
	}

	function deleteGroup(id: number, name: string) {
		confirmDlg = { message: $t('groups.deleteConfirm'), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/groups/${id}`);
			if (r.success) { toasts.success($t('common.delete')); loadAll(); } else toasts.error(r.error || $t('common.error'));
		}};
	}
</script>

<svelte:head><title>DockPit — {$t('nav.userManagement')}</title></svelte:head>

<div class="space-y-4">
	<div class="flex items-center justify-between flex-wrap gap-3">
		<div>
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('nav.userManagement')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{users.length} {$t('nav.users')} · {groups.length} {$t('groups.title')}</p>
		</div>
	</div>

	<div class="bg-card border border-theme rounded-lg overflow-hidden">
		<Tabs tabs={tabs} active={activeTab} onchange={(id) => activeTab = Number(id)} />

		<div class="p-5">
			{#if loading}
				<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>

			<!-- Tab 0: Users -->
			{:else if activeTab === 0}
				<div class="flex items-center justify-between mb-4 flex-wrap gap-3">
					<div class="flex items-center gap-3">
						{#each roles as r}
							<div class="flex items-center gap-1.5">
								<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium {roleBadge(r.value)}">{r.label}</span>
								<span class="text-[10px] text-muted">{users.filter(u => u.role === r.value).length}</span>
							</div>
						{/each}
					</div>
					<Button variant="primary" size="sm" onclick={() => showCreate = true}>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
						{$t('users.createUser')}
					</Button>
				</div>

				<div class="overflow-x-auto">
					<table class="w-full">
						<thead><tr class="border-b border-theme">
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('login.username')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('users.role')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('groups.title')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">2FA</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">Email</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('users.created')}</th>
							<th class="text-left px-4 py-2 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
						</tr></thead>
						<tbody>
							{#each users as u}
								<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
									<td class="px-4 py-2.5 text-sm font-medium text-primary">{u.username}</td>
									<td class="px-4 py-2.5"><span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium {roleBadge(u.role)}">{roleLabel(u.role)}</span></td>
									<td class="px-4 py-2.5">
										<div class="flex flex-wrap gap-1">
											{#if u.groups && u.groups.length > 0}
												{#each u.groups as g}
													<span class="px-1.5 py-0.5 rounded text-[9px] font-medium text-white" style="background-color: {g.color || '#6c5ce7'}">{g.name}</span>
												{/each}
											{:else}
												<span class="text-[10px] text-muted">—</span>
											{/if}
										</div>
									</td>
									<td class="px-4 py-2.5">
										{#if u.totp_enabled}
											<span class="w-4 h-4 rounded-full bg-[var(--green)] flex items-center justify-center"><svg class="w-2.5 h-2.5 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg></span>
										{:else}
											<span class="text-[10px] text-muted">—</span>
										{/if}
									</td>
									<td class="px-4 py-2.5 text-[11px] text-muted hidden md:table-cell">{u.email || '—'}</td>
									<td class="px-4 py-2.5 text-[11px] text-muted hidden md:table-cell">{formatDateTime(u.created_at)}</td>
									<td class="px-4 py-2.5">
										<div class="flex gap-1">
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)] transition" title={$t('groups.assignGroups')} onclick={() => openAssignGroups(u)}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
											</button>
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-secondary hover:text-primary hover:border-light transition" title={$t('common.edit')} onclick={() => { editUser = u; editRole = u.role; editPass = ''; }}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
											</button>
											{#if u.totp_enabled}
												<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--yellow)] hover:border-[var(--yellow)] transition" title="Reset 2FA" onclick={() => resetMfa(u.id)}>
													<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
												</button>
											{/if}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme transition {canDeleteUser(u) ? 'text-[var(--red)] hover:border-[var(--red)]' : 'text-muted opacity-40 cursor-not-allowed'}" title={$t('common.delete')} onclick={() => canDeleteUser(u) && deleteUser(u.id, u.username)} disabled={!canDeleteUser(u)}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
											</button>
										</div>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>

				<!-- Role descriptions -->
				<div class="mt-5 pt-4 border-t border-theme">
					<h4 class="text-xs font-semibold text-primary mb-3">{$t('users.permissions')}</h4>
					<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
						{#each roles as r}
							<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-3">
								<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium {roleBadge(r.value)} mb-1">{r.label}</span>
								<p class="text-[10px] text-muted">{r.desc}</p>
							</div>
						{/each}
					</div>
				</div>

			<!-- Tab 1: Groups -->
			{:else if activeTab === 1}
				<div class="flex items-center justify-between mb-4">
					<p class="text-xs text-secondary">{$t('groups.title')}</p>
					<Button variant="primary" size="sm" onclick={() => showCreateGroup = true}>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
						{$t('groups.create')}
					</Button>
				</div>

				{#if groups.length === 0}
					<div class="text-center py-10 text-sm text-muted">{$t('groups.noGroups')}</div>
				{:else}
					<div class="space-y-3">
						{#each groups as g}
							<div class="bg-[var(--bg-0)] border border-theme rounded-lg p-4">
								<div class="flex items-center justify-between mb-2">
									<div class="flex items-center gap-2">
										<span class="w-3 h-3 rounded-full shrink-0" style="background-color: {g.color || '#6c5ce7'}"></span>
										<h4 class="text-sm font-semibold text-primary">{g.name}</h4>
										{#if g.is_default}
											<span class="px-1.5 py-0.5 rounded text-[8px] font-medium uppercase bg-[color-mix(in_srgb,var(--accent),transparent_85%)] text-accent border border-[color-mix(in_srgb,var(--accent),transparent_70%)]">{$t('groups.default')}</span>
										{/if}
									</div>
									<div class="flex items-center gap-1.5">
										<Button variant="secondary" size="sm" onclick={() => openEditGroupInfo(g)}>
											<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
											{$t('common.edit')}
										</Button>
										<Button variant="secondary" size="sm" onclick={() => openEditGroup(g)}>
											{$t('groups.permissions')}
										</Button>
										{#if !g.is_default}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)] transition" onclick={() => deleteGroup(g.id, g.name)}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
											</button>
										{/if}
									</div>
								</div>
								{#if groupDesc(g)}
									<p class="text-[11px] text-muted mb-2">{groupDesc(g)}</p>
								{/if}
								<div class="flex items-center gap-3 text-[10px] text-secondary">
									<span>{$t('groups.members')}: {g.member_count}</span>
									<span>{$t('groups.permissions')}: {g.permissions?.length || 0}</span>
								</div>
								{#if g.members && g.members.length > 0}
									<div class="flex flex-wrap gap-1 mt-2">
										{#each g.members as m}
											<span class="px-1.5 py-0.5 rounded text-[9px] font-medium bg-card border border-theme text-primary">{m.username}</span>
										{/each}
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			{/if}
		</div>
	</div>
</div>

<!-- Create User Modal -->
{#if showCreate}
	<Modal title={$t('users.createUser')} onclose={() => showCreate = false}>
		<form onsubmit={createUser} class="space-y-3">
			<TextInput bind:value={newUser} label={$t('login.username')} placeholder="min. 3" required id="nu" />
			<TextInput bind:value={newPass} type="password" label={$t('login.password')} placeholder="min. 6" required id="np" />
			<div>
				<label class="block text-xs font-medium text-secondary mb-1">{$t('users.role')}</label>
				<CustomSelect options={roles.map(r => ({ value: r.value, label: r.label }))} value={newRole} onchange={(v) => newRole = String(v)} />
			</div>
			<p class="text-[10px] text-muted">{roles.find(r => r.value === newRole)?.desc}</p>
		</form>
		<div class="flex justify-end gap-2 mt-4">
			<Button variant="danger" size="sm" onclick={() => showCreate = false}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={createUser} loading={creating}>{$t('common.create')}</Button>
		</div>
	</Modal>
{/if}

<!-- Edit User Modal -->
{#if editUser}
	<Modal title={$t('common.edit') + ': ' + editUser.username} onclose={() => editUser = null}>
		<form onsubmit={saveUser} class="space-y-3">
			<div>
				<label class="block text-xs font-medium text-secondary mb-1">{$t('users.role')}</label>
				<CustomSelect options={roles.map(r => ({ value: r.value, label: r.label }))} value={editRole} onchange={(v) => editRole = String(v)} />
			</div>
			<TextInput bind:value={editPass} type="password" label={$t('login.password') + ' (optional)'} placeholder={$t('users.leaveEmpty')} id="ep" />
		</form>
		<div class="flex justify-end gap-2 mt-4">
			<Button variant="danger" size="sm" onclick={() => editUser = null}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={saveUser} loading={editSaving}>{$t('common.save')}</Button>
		</div>
	</Modal>
{/if}

<!-- Assign Groups Modal -->
{#if assignGroupsUser}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1000] p-0 sm:p-4" onclick={(e) => { if (e.target === e.currentTarget) { assignGroupsUser = null; assignDropdownOpen = false; } }}>
		<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-lg shadow-[var(--shadow-lg)]" style="overflow:visible; background:var(--glass-bg); backdrop-filter:blur(20px) saturate(150%); -webkit-backdrop-filter:blur(20px) saturate(150%)">
			<div class="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
				<h3 class="text-[15px] font-semibold text-[var(--text)]">{$t('groups.assignGroups')}: {assignGroupsUser.username}</h3>
				<button class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all" onclick={() => { assignGroupsUser = null; assignDropdownOpen = false; }}>
					<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>
			<div class="p-5 space-y-3">
				<div class="relative">
					<button
						class="w-full flex items-center gap-2.5 h-9 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-3)] text-[var(--text)] hover:border-[var(--border-light)] hover:shadow-[var(--shadow-sm)] transition-all cursor-pointer"
						onclick={(e) => { e.stopPropagation(); assignDropdownOpen = !assignDropdownOpen; }}
					>
						{#if assignGroupIds.length > 0}
							<div class="flex flex-wrap gap-1 flex-1">
								{#each groups.filter(g => assignGroupIds.includes(g.id)) as g}
									<span class="px-1.5 py-0.5 rounded text-[9px] font-medium text-white" style="background-color: {g.color || '#6c5ce7'}">{g.name}</span>
								{/each}
							</div>
						{:else}
							<span class="text-muted">{$t('groups.assignGroups')}</span>
						{/if}
						<svg class="w-3.5 h-3.5 text-muted shrink-0 ml-auto transition-transform duration-200 {assignDropdownOpen ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
					</button>
					{#if assignDropdownOpen}
						<div class="absolute left-0 right-0 mt-1.5 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] z-[1100] py-1.5">
							{#each groups as g}
								<button
									class="w-full flex items-center gap-3 px-3 py-2.5 text-xs text-left transition-all duration-150
									{assignGroupIds.includes(g.id) ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text)]'}"
									onclick={() => toggleGroupAssign(g.id)}
								>
									<span class="w-3 h-3 rounded-full shrink-0" style="background-color: {g.color || '#6c5ce7'}"></span>
									<span class="truncate font-medium">{g.name}</span>
									{#if assignGroupIds.includes(g.id)}
										<svg class="w-3.5 h-3.5 shrink-0 ml-auto text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
									{/if}
								</button>
							{/each}
						</div>
					{/if}
				</div>
			</div>
			<div class="px-5 py-3 border-t border-[var(--border)] flex justify-end gap-2">
			<Button variant="danger" size="sm" onclick={() => { assignGroupsUser = null; assignDropdownOpen = false; }}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={saveGroupAssignment}>{$t('common.save')}</Button>
		</div>
	</div>
	</div>
{/if}

<!-- Create Group Modal -->
{#if showCreateGroup}
	<Modal title={$t('groups.create')} onclose={() => showCreateGroup = false}>
		<form onsubmit={createGroup} class="space-y-3">
			<TextInput bind:value={newGroupName} label={$t('groups.name')} required id="gn" />
			<TextInput bind:value={newGroupDesc} label={$t('groups.description')} id="gd" />
			<div>
				<label class="block text-xs font-medium text-secondary mb-1.5">{$t('groups.color')}</label>
				<div class="flex items-center gap-3">
					<input type="color" bind:value={newGroupColor} class="w-10 h-10 rounded-lg border border-theme cursor-pointer bg-transparent p-0.5" />
					<span class="text-xs font-mono text-muted">{newGroupColor}</span>
					<span class="px-2 py-0.5 rounded text-[10px] font-medium text-white" style="background-color: {newGroupColor}">Preview</span>
				</div>
			</div>
		</form>
		<div class="flex justify-end gap-2 mt-4">
			<Button variant="danger" size="sm" onclick={() => showCreateGroup = false}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={createGroup}>{$t('common.create')}</Button>
		</div>
	</Modal>
{/if}

{#if editGroupInfo}
	<Modal title={$t('common.edit') + ': ' + editGroupInfo.name} onclose={() => editGroupInfo = null}>
		<div class="space-y-3">
			<TextInput bind:value={editGroupName} label={$t('groups.name')} required id="egn" />
			<TextInput bind:value={editGroupDesc} label={$t('groups.description')} id="egd" />
			<div>
				<label class="block text-xs font-medium text-secondary mb-1.5">{$t('groups.color')}</label>
				<div class="flex items-center gap-3">
					<input type="color" bind:value={editGroupColor} class="w-10 h-10 rounded-lg border border-theme cursor-pointer bg-transparent p-0.5" />
					<span class="text-xs font-mono text-muted">{editGroupColor}</span>
					<span class="px-2 py-0.5 rounded text-[10px] font-medium text-white" style="background-color: {editGroupColor}">Preview</span>
				</div>
			</div>
		</div>
		<div class="flex justify-end gap-2 mt-4">
			<Button variant="danger" size="sm" onclick={() => editGroupInfo = null}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={saveGroupInfo}>{$t('common.save')}</Button>
		</div>
	</Modal>
{/if}

<!-- Edit Group Permissions Modal -->
{#if editGroup}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1000] p-0 sm:p-4" onclick={(e) => { if (e.target === e.currentTarget) editGroup = null; }}>
		<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-2xl max-h-[90vh] sm:max-h-[85vh] flex flex-col shadow-[var(--shadow-lg)]" style="background:var(--glass-bg); backdrop-filter:blur(20px) saturate(150%); -webkit-backdrop-filter:blur(20px) saturate(150%)">
			<div class="flex items-center justify-between px-5 py-4 border-b border-[var(--border)] shrink-0">
				<div class="flex items-center gap-2">
					<span class="w-3 h-3 rounded-full" style="background-color: {editGroup.color || '#6c5ce7'}"></span>
					<h3 class="text-[15px] font-semibold text-[var(--text)]">{$t('groups.permissions')}: {editGroup.name}</h3>
				</div>
				<div class="flex items-center gap-2">
					<button class="px-2 py-1 text-[10px] font-medium rounded-md bg-[var(--accent-bg)] text-accent hover:bg-[var(--accent)]/20 transition" onclick={selectAllPerms}>Alle</button>
					<button class="px-2 py-1 text-[10px] font-medium rounded-md bg-[var(--bg-hover)] text-muted hover:text-primary transition" onclick={selectNonePerms}>Keine</button>
					<button class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all" onclick={() => editGroup = null}>
						<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					</button>
				</div>
			</div>
			<div class="flex-1 overflow-y-auto px-5 py-3">
				<div class="space-y-1">
					{#each permSections as section}
						{@const pageActive = editPerms.includes(section.page)}
						{@const actionCount = section.actions.filter(a => editPerms.includes(a.key)).length}
						{@const isExpanded = expandedSections.has(section.page)}
						<div class="border border-theme rounded-lg overflow-hidden {pageActive ? 'border-[var(--accent)]/30' : ''}">
							<div class="flex items-center gap-3 px-3 py-2.5 bg-[var(--bg-0)] {section.actions.length > 0 ? 'cursor-pointer' : ''}" onclick={() => section.actions.length > 0 && toggleSection(section.page)}>
								<CustomCheckbox checked={pageActive} onchange={() => togglePageWithSuggestions(section.page, section.actions)} />
								<span class="text-sm">{section.icon}</span>
								<span class="text-xs font-medium text-primary flex-1">{section.label}</span>
								{#if section.actions.length > 0}
									<span class="text-[9px] text-muted px-1.5 py-0.5 rounded-full bg-[var(--bg-3)]">{actionCount}/{section.actions.length}</span>
									<svg class="w-3 h-3 text-muted transition-transform {isExpanded ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
								{/if}
							</div>
							{#if isExpanded && section.actions.length > 0}
								<div class="px-3 py-2 border-t border-theme bg-card">
									<div class="flex items-center justify-between ml-8 mb-1.5">
										<span class="text-[9px] text-muted uppercase tracking-wider">{$t('groups.actionPermissions')}</span>
										<button class="px-2 py-0.5 text-[9px] font-medium rounded-md transition {section.actions.every(a => editPerms.includes(a.key)) ? 'bg-[var(--red-bg)] text-[var(--red)]' : 'bg-[var(--accent-bg)] text-accent'}" onclick={() => toggleAllSectionActions(section.page, section.actions)}>
											{section.actions.every(a => editPerms.includes(a.key)) ? 'Keine' : 'Alle'}
										</button>
									</div>
									<div class="grid grid-cols-2 gap-1.5 ml-8">
										{#each section.actions as action}
											<CustomCheckbox checked={editPerms.includes(action.key)} onchange={() => togglePerm(action.key)} label={action.label} size="sm" />
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{/each}
				</div>

				<!-- System permissions -->
				<div class="mt-3 pt-3 border-t border-theme">
					<h4 class="text-[10px] font-semibold uppercase tracking-wider text-muted mb-2">System</h4>
					<div class="grid grid-cols-2 gap-1.5">
						{#each systemPerms as p}
							<CustomCheckbox checked={editPerms.includes(p.key)} onchange={() => togglePerm(p.key)} label={p.label} />
						{/each}
					</div>
				</div>
			</div>
			<div class="px-5 py-3 border-t border-[var(--border)] flex justify-between items-center shrink-0">
				<span class="text-[10px] text-muted">{editPerms.length} {$t('groups.permissions')}</span>
				<div class="flex gap-2">
					<Button variant="danger" size="sm" onclick={() => editGroup = null}>{$t('common.cancel')}</Button>
					<Button variant="primary" size="sm" onclick={saveGroupPerms}>{$t('common.save')}</Button>
				</div>
			</div>
		</div>
	</div>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
