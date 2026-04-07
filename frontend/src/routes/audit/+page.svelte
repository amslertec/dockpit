<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { canSeePage } from '$lib/stores/auth';
	import { api } from '$lib/api/client';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import { formatDateTime } from '$lib/utils/format';
	import { initResizableColumns } from '$lib/utils/resizable-columns';
	import type { AuditEntry, AuditResponse } from '$lib/api/types';

	$effect(() => {
		if (!$canSeePage('page.audit')) goto('/profile');
	});

	$effect(() => { if (tableEl && !loading && entries.length > 0) initResizableColumns(tableEl); });

	let tableEl: HTMLTableElement | undefined = $state();
	let entries = $state<AuditEntry[]>([]);
	let loading = $state(true);
	let total = $state(0);
	let page = $state(1);
	let perPage = $state(25);
	let filterUser = $state('');
	let filterAction = $state('');

	const actionOptions = $derived([
		{ value: '', label: $t('audit.allActions') },
		{ value: 'login', label: $t('audit.login') },
		{ value: 'login_failed', label: $t('audit.loginFailed') },
		{ value: 'container_action', label: $t('audit.containerAction') },
		{ value: 'container_recreate', label: $t('audit.containerRecreate') },
		{ value: 'stack_deploy', label: $t('audit.stackDeploy') },
		{ value: 'stack_stop', label: $t('audit.stackStop') },
		{ value: 'stack_create', label: $t('audit.stackCreate') },
		{ value: 'stack_delete', label: $t('audit.stackDelete') },
		{ value: 'user_create', label: $t('audit.userCreate') },
		{ value: 'user_delete', label: $t('audit.userDelete') },
		{ value: 'settings_update', label: $t('audit.settingsUpdate') },
		{ value: 'env_create', label: $t('audit.envCreate') },
		{ value: 'env_delete', label: $t('audit.envDelete') },
		{ value: 'vuln_scan', label: $t('audit.vulnScan') },
		{ value: 'update_check', label: $t('audit.updateCheck') },
		{ value: 'job_create', label: $t('audit.jobCreate') },
		{ value: 'job_delete', label: $t('audit.jobDelete') },
		{ value: 'container_migrate', label: $t('audit.containerMigrate') },
		{ value: 'container_restart', label: $t('audit.containerRestart') },
		{ value: 'container_remove', label: $t('audit.containerRemove') },
		{ value: 'container_start', label: $t('audit.containerStart') },
		{ value: 'container_stop', label: $t('audit.containerStop') },
		{ value: 'container_rollback', label: $t('audit.containerRollback') },
		{ value: 'backup_scheduled', label: $t('audit.backupScheduled') },
		{ value: 'backup_create', label: $t('audit.backupCreate') },
		{ value: 'template_create', label: $t('audit.templateCreate') },
		{ value: 'login_blocked', label: $t('audit.loginBlocked') },
		{ value: 'group_create', label: $t('audit.groupCreate') },
		{ value: 'group_delete', label: $t('audit.groupDelete') },
		{ value: 'stack_migrate', label: $t('audit.stackMigrate') },
	]);

	onMount(() => {
		loadEntries();
	});

	let prevFilterUser = $state('');
	let prevFilterAction = $state('');
	let prevPage = $state(1);
	let prevPerPage = $state(25);

	$effect(() => {
		const filterChanged = filterUser !== prevFilterUser || filterAction !== prevFilterAction;
		const pageChanged = page !== prevPage || perPage !== prevPerPage;

		if (filterChanged) {
			prevFilterUser = filterUser;
			prevFilterAction = filterAction;
			if (page !== 1) {
				page = 1;
				prevPage = 1;
			}
			loadEntries();
		} else if (pageChanged) {
			prevPage = page;
			prevPerPage = perPage;
			loadEntries();
		}
	});

	async function loadEntries() {
		loading = true;
		const offset = (page - 1) * perPage;
		const params = new URLSearchParams({
			limit: String(perPage),
			offset: String(offset),
			user: filterUser,
			action: filterAction,
		});
		const r = await api.get<AuditResponse>(`/audit?${params}`);
		if (r.success && r.data) {
			entries = r.data.entries || [];
			total = r.data.total;
		}
		loading = false;
	}

	function handlePageChange(p: number, pp: number) {
		page = p;
		perPage = pp;
	}

	function badgeClass(action: string): string {
		const map: Record<string, string> = {
			login: 'bg-[var(--green)]/15 text-[var(--green)] border border-[var(--green)]/25',
			login_failed: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			container_action: 'bg-[var(--accent)]/15 text-[var(--accent)] border border-[var(--accent)]/25',
			container_recreate: 'bg-[var(--purple)]/15 text-[var(--purple)] border border-[var(--purple)]/25',
			container_migrate: 'bg-cyan-400/15 text-cyan-400 border border-cyan-400/25',
			stack_deploy: 'bg-[var(--green)]/15 text-[var(--green)] border border-[var(--green)]/25',
			stack_stop: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			stack_create: 'bg-[var(--accent)]/15 text-[var(--accent)] border border-[var(--accent)]/25',
			stack_delete: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			user_create: 'bg-[var(--green)]/15 text-[var(--green)] border border-[var(--green)]/25',
			user_delete: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			settings_update: 'bg-[var(--yellow)]/15 text-[var(--yellow)] border border-[var(--yellow)]/25',
			env_create: 'bg-[var(--accent)]/15 text-[var(--accent)] border border-[var(--accent)]/25',
			env_delete: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			vuln_scan: 'bg-[var(--purple)]/15 text-[var(--purple)] border border-[var(--purple)]/25',
			update_check: 'bg-[var(--yellow)]/15 text-[var(--yellow)] border border-[var(--yellow)]/25',
			job_create: 'bg-[var(--accent)]/15 text-[var(--accent)] border border-[var(--accent)]/25',
			job_delete: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			container_restart: 'bg-[var(--yellow)]/15 text-[var(--yellow)] border border-[var(--yellow)]/25',
			container_remove: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			container_start: 'bg-[var(--green)]/15 text-[var(--green)] border border-[var(--green)]/25',
			container_stop: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			container_rollback: 'bg-[var(--yellow)]/15 text-[var(--yellow)] border border-[var(--yellow)]/25',
			backup_scheduled: 'bg-[var(--green)]/15 text-[var(--green)] border border-[var(--green)]/25',
			backup_create: 'bg-[var(--green)]/15 text-[var(--green)] border border-[var(--green)]/25',
			template_create: 'bg-[var(--purple)]/15 text-[var(--purple)] border border-[var(--purple)]/25',
			login_blocked: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			group_create: 'bg-[var(--accent)]/15 text-[var(--accent)] border border-[var(--accent)]/25',
			group_delete: 'bg-[var(--red)]/15 text-[var(--red)] border border-[var(--red)]/25',
			stack_migrate: 'bg-cyan-400/15 text-cyan-400 border border-cyan-400/25',
		};
		return map[action] || 'bg-[var(--bg-hover)] text-[var(--text-secondary)] border border-[var(--border)]';
	}

	function actionLabel(action: string): string {
		const keyMap: Record<string, string> = {
			login: 'audit.login',
			login_failed: 'audit.loginFailed',
			container_action: 'audit.containerAction',
			container_recreate: 'audit.containerRecreate',
			stack_deploy: 'audit.stackDeploy',
			stack_stop: 'audit.stackStop',
			stack_create: 'audit.stackCreate',
			stack_delete: 'audit.stackDelete',
			user_create: 'audit.userCreate',
			user_delete: 'audit.userDelete',
			settings_update: 'audit.settingsUpdate',
			env_create: 'audit.envCreate',
			env_delete: 'audit.envDelete',
			vuln_scan: 'audit.vulnScan',
			update_check: 'audit.updateCheck',
			job_create: 'audit.jobCreate',
			job_delete: 'audit.jobDelete',
			container_migrate: 'audit.containerMigrate',
			container_restart: 'audit.containerRestart',
			container_remove: 'audit.containerRemove',
			container_start: 'audit.containerStart',
			container_stop: 'audit.containerStop',
			container_rollback: 'audit.containerRollback',
			backup_scheduled: 'audit.backupScheduled',
			backup_create: 'audit.backupCreate',
			template_create: 'audit.templateCreate',
			login_blocked: 'audit.loginBlocked',
			group_create: 'audit.groupCreate',
			group_delete: 'audit.groupDelete',
			stack_migrate: 'audit.stackMigrate',
		};
		const key = keyMap[action];
		if (key) {
			const translated = $t(key);
			return translated !== key ? translated : action;
		}
		return action;
	}
</script>

<div class="space-y-4">
	<!-- Header -->
	<div class="flex items-center justify-between flex-wrap gap-3">
		<div>
			<h1 class="text-xl font-bold text-[var(--text)]">{$t('audit.title')}</h1>
			<p class="text-xs text-[var(--text-muted)] mt-0.5">{total} {$t('audit.title').toLowerCase()}</p>
		</div>
	</div>

	<!-- Filter bar -->
	<div class="flex items-center gap-3 flex-wrap">
		<input
			type="text"
			placeholder={$t('audit.allUsers')}
			bind:value={filterUser}
			class="h-8 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-card)] text-[var(--text)] placeholder:text-[var(--text-muted)] focus:outline-none focus:border-[var(--accent)] w-[180px]"
		/>
		<CustomSelect
			options={actionOptions}
			value={filterAction}
			onchange={(v) => { filterAction = String(v); }}
			size="sm"
			class="w-[200px]"
		/>
	</div>

	<!-- Table -->
	<div class="bg-card rounded-[var(--radius-lg)] border border-[var(--border)] overflow-hidden">
		{#if loading}
			<div class="flex items-center justify-center py-16">
				<div class="w-6 h-6 border-2 border-[var(--border)] border-t-[var(--accent)] rounded-full animate-spin"></div>
			</div>
		{:else if entries.length === 0}
			<div class="flex flex-col items-center justify-center py-16 text-center">
				<svg class="w-10 h-10 text-[var(--text-muted)] mb-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2"/><rect x="9" y="3" width="6" height="4" rx="1"/><path d="M9 14l2 2 4-4"/></svg>
				<p class="text-sm font-medium text-[var(--text-secondary)]">{$t('audit.noEntries')}</p>
				<p class="text-xs text-[var(--text-muted)] mt-1 max-w-[300px]">{$t('audit.noEntriesDesc')}</p>
			</div>
		{:else}
			<div class="overflow-x-auto">
				<table bind:this={tableEl} class="w-full text-sm">
					<thead>
						<tr class="border-b border-[var(--border)]">
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('audit.time')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('audit.user')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('audit.action')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('audit.target')}</th>
							<th class="text-left px-4 py-3 text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">{$t('audit.details')}</th>
						</tr>
					</thead>
					<tbody>
						{#each entries as entry (entry.id)}
							<tr class="border-b border-[var(--border)] last:border-0 hover:bg-[var(--bg-hover)] transition-colors duration-150">
								<td class="px-4 py-3 text-xs text-[var(--text-muted)] font-mono whitespace-nowrap">{formatDateTime(entry.created_at)}</td>
								<td class="px-4 py-3 text-xs text-[var(--text)] font-medium">{entry.username}</td>
								<td class="px-4 py-3">
									<span class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium {badgeClass(entry.action)}">
										{actionLabel(entry.action)}
									</span>
								</td>
								<td class="px-4 py-3 text-xs text-[var(--text-secondary)]">{entry.target || '-'}</td>
								<td class="px-4 py-3 text-xs text-[var(--text-secondary)] max-w-[300px] truncate">{entry.details || '-'}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>

			<Pagination {total} {page} {perPage} onchange={handlePageChange} />
		{/if}
	</div>
</div>
