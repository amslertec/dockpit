<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import type { UserInfo } from '$lib/api/types';

	let users = $state<UserInfo[]>([]);
	let loading = $state(true);
	let search = $state('');
	let page = $state(1);
	let perPage = $state(10);

	// Create user
	let showCreate = $state(false);
	let newUser = $state('');
	let newPass = $state('');
	let newRole = $state('viewer');
	let creating = $state(false);

	// Edit user
	let editUser = $state<UserInfo | null>(null);
	let editRole = $state('');
	let editPass = $state('');
	let editSaving = $state(false);

	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);

	const roles = [
		{ value: 'super_admin', label: 'Super Admin', desc: $t('users.descSuperAdmin') },
		{ value: 'admin', label: 'Admin', desc: $t('users.descAdmin') },
		{ value: 'editor', label: 'Editor', desc: $t('users.descEditor') },
		{ value: 'viewer', label: 'Viewer', desc: $t('users.descViewer') },
	];

	const filtered = $derived(users.filter(u => u.username.toLowerCase().includes(search.toLowerCase())));
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));

	function roleBadge(role: string): string {
		const m: Record<string, string> = { super_admin: 'bg-purple-light text-purple', admin: 'bg-accent-light text-accent', editor: 'bg-yellow-light text-yellow', viewer: 'bg-3 text-secondary' };
		return m[role] || 'bg-3 text-secondary';
	}

	function roleLabel(role: string): string {
		return roles.find(r => r.value === role)?.label || role;
	}

	onMount(() => load());

	async function load() {
		loading = true;
		const r = await api.get<UserInfo[]>('/users');
		if (r.success && r.data) users = r.data;
		else if (r.error?.includes('Berechtigung')) toasts.error($t('users.noPermission'));
		loading = false;
	}

	async function createUser(e: Event) {
		e.preventDefault();
		creating = true;
		const r = await api.post<string>('/users', { username: newUser, password: newPass, role: newRole });
		creating = false;
		if (r.success) { showCreate = false; newUser = ''; newPass = ''; newRole = 'viewer'; toasts.success($t('users.userCreated')); load(); }
		else toasts.error(r.error || $t('common.error'));
	}

	function openEdit(u: UserInfo) { editUser = u; editRole = u.role; editPass = ''; }

	async function saveEdit(e: Event) {
		e.preventDefault();
		if (!editUser) return;
		editSaving = true;
		const body: any = { role: editRole };
		if (editPass) body.password = editPass;
		const r = await api.put<string>(`/users/${editUser.id}`, body);
		editSaving = false;
		if (r.success) { editUser = null; toasts.success($t('common.save')); load(); }
		else toasts.error(r.error || $t('common.error'));
	}

	const superAdminCount = $derived(users.filter(u => u.role === 'super_admin').length);

	function canDelete(u: UserInfo): boolean {
		// Cannot delete last super_admin
		if (u.role === 'super_admin' && superAdminCount <= 1) return false;
		return true;
	}

	function deleteUser(u: UserInfo) {
		confirmDlg = { message: $t('users.confirmDelete', { name: u.username }), action: async () => {
			confirmDlg = null;
			const r = await api.del<string>(`/users/${u.id}`);
			if (r.success) { toasts.success($t('common.delete')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function resetMfa(u: UserInfo) {
		confirmDlg = { message: $t('users.confirmReset2FA', { name: u.username }), action: async () => {
			confirmDlg = null;
			const r = await api.post<string>(`/users/${u.id}/reset-mfa`, {});
			if (r.success) { toasts.success($t('users.reset2FA')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — {$t('users.title')}</title></svelte:head>

{#if !loading}
	<div class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{users.length}</div>
			<div class="text-[11px] text-secondary">{$t('users.total')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-purple">{users.filter(u => u.role === 'super_admin').length}</div>
			<div class="text-[11px] text-secondary">{$t('users.superAdmins')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{users.filter(u => u.role === 'admin').length}</div>
			<div class="text-[11px] text-secondary">{$t('users.admins')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{users.filter(u => u.totp_enabled).length}</div>
			<div class="text-[11px] text-secondary">{$t('users.with2FA')}</div>
		</div>
	</div>
{/if}

<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<h3 class="text-sm font-semibold text-primary">{$t('users.title')}</h3>
		<div class="flex items-center gap-2">
			<input bind:value={search} placeholder={$t('common.search')} class="bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs w-44 focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200" />
			<Button variant="primary" size="sm" onclick={() => showCreate = true}><svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>{$t('users.createUser')}</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('login.username')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('users.role')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('users.2fa')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell">{$t('users.created')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as u}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
							<td class="px-4 py-3 text-sm font-medium text-primary">{u.username}</td>
							<td class="px-4 py-3">
								<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium {roleBadge(u.role)}">
									{roleLabel(u.role)}
								</span>
							</td>
							<td class="px-4 py-3">
								{#if u.totp_enabled}
									<span class="inline-flex items-center gap-1 text-[10px] text-green"><span class="w-1.5 h-1.5 rounded-full bg-current"></span>{$t('users.active')}</span>
								{:else}
									<span class="text-[10px] text-muted">—</span>
								{/if}
							</td>
							<td class="px-4 py-3 text-xs text-secondary hidden md:table-cell">{u.created_at?.substring(0, 10) || '—'}</td>
							<td class="px-4 py-3">
								<div class="flex gap-1">
									<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition" title={$t('common.edit')} onclick={() => openEdit(u)}>
										<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
									</button>
									{#if u.totp_enabled}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--yellow)] hover:border-[var(--yellow)]/40 hover:bg-[var(--yellow)]/8 transition" title={$t('users.reset2FA')} onclick={() => resetMfa(u)}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><line x1="9" y1="12" x2="15" y2="12"/></svg>
										</button>
									{/if}
									{#if canDelete(u)}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" title={$t('common.delete')} onclick={() => deleteUser(u)}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
										</button>
									{:else}
										<button disabled class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-muted opacity-30 cursor-not-allowed" title={$t('users.lastSuperAdmin')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
										</button>
									{/if}
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="5" class="text-center py-10 text-sm text-muted">{$t('users.noUsers')}</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
		<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
	{/if}
</div>

<!-- Permission Matrix -->
<div class="bg-card border border-theme rounded-lg overflow-hidden mt-4">
	<div class="px-4 py-3 border-b border-theme"><h3 class="text-sm font-semibold text-primary">{$t('users.permissionMatrix')}</h3></div>
	<div class="overflow-x-auto">
		<table class="w-full text-xs">
			<thead><tr class="border-b border-theme">
				<th class="text-left px-4 py-2 text-[10px] text-muted font-semibold">{$t('users.action')}</th>
				<th class="text-center px-2 py-2 text-[10px] text-purple font-semibold">{$t('users.roleSuperAdmin')}</th>
				<th class="text-center px-2 py-2 text-[10px] text-accent font-semibold">{$t('users.roleAdmin')}</th>
				<th class="text-center px-2 py-2 text-[10px] text-yellow font-semibold">{$t('users.roleEditor')}</th>
				<th class="text-center px-2 py-2 text-[10px] text-secondary font-semibold">{$t('users.roleViewer')}</th>
			</tr></thead>
			<tbody>
				{#each [
					[$t('users.permViewDashboard'), true, true, true, true],
					[$t('users.permViewContainers'), true, true, true, true],
					[$t('users.permStartStop'), true, true, true, false],
					[$t('users.permRecreateDelete'), true, true, false, false],
					[$t('users.permTerminalLogs'), true, true, false, false],
					[$t('users.permStacksEdit'), true, true, false, false],
					[$t('users.permStacksDeploy'), true, true, true, false],
					[$t('users.permResourcesDelete'), true, true, false, false],
					[$t('users.permEnvManage'), true, true, false, false],
					[$t('users.permSettings'), true, true, false, false],
					[$t('users.permUserManage'), true, false, false, false],
				] as [label, sa, a, e, v]}
					<tr class="border-b border-theme last:border-0">
						<td class="px-4 py-1.5 text-secondary">{label}</td>
						<td class="text-center">{sa ? '✓' : '—'}</td>
						<td class="text-center">{a ? '✓' : '—'}</td>
						<td class="text-center">{e ? '✓' : '—'}</td>
						<td class="text-center">{v ? '✓' : '—'}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>

<!-- Create User Modal -->
{#if showCreate}
	<Modal title={$t('users.createUser')} onclose={() => showCreate = false}>
		<form onsubmit={createUser} class="space-y-3">
			<TextInput bind:value={newUser} label={$t('login.username')} placeholder={$t('users.minChars', { count: '3' })} required id="nu" />
			<TextInput bind:value={newPass} type="password" label={$t('login.password')} placeholder={$t('users.minChars', { count: '6' })} required id="np" />
			<div>
				<div class="block text-xs font-medium text-secondary mb-1">{$t('users.role')}</div>
				<CustomSelect options={roles.map(r => ({value: r.value, label: r.label + ' — ' + r.desc}))} value={newRole} onchange={(v) => newRole = String(v)} />
			</div>
			<div class="flex justify-end gap-2 pt-2">
				<Button variant="danger" size="sm" onclick={() => showCreate = false}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" type="submit" loading={creating}>{$t('common.create')}</Button>
			</div>
		</form>
	</Modal>
{/if}

<!-- Edit User Modal -->
{#if editUser}
	<Modal title={$t('users.editUser', { name: editUser.username })} onclose={() => editUser = null}>
		<form onsubmit={saveEdit} class="space-y-3">
			<div>
				<div class="block text-xs font-medium text-secondary mb-1">{$t('users.role')}</div>
				<CustomSelect options={roles.map(r => ({value: r.value, label: r.label + ' — ' + r.desc}))} value={editRole} onchange={(v) => editRole = String(v)} />
			</div>
			<TextInput bind:value={editPass} type="password" label={$t('users.newPassword')} placeholder={$t('users.newPasswordHint')} id="ep" />
			<div class="flex justify-end gap-2 pt-2">
				<Button variant="danger" size="sm" onclick={() => editUser = null}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" type="submit" loading={editSaving}>{$t('common.save')}</Button>
			</div>
		</form>
	</Modal>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
