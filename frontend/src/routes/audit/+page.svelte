<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$lib/api/client';
	import { t } from '$lib/i18n';
	import Button from '$lib/components/ui/Button.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import CustomSelect from '$lib/components/ui/CustomSelect.svelte';
	import { formatDateTime } from '$lib/utils/format';
	import type { AuditEntry, AuditResponse } from '$lib/api/types';

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
	]);

	onMount(() => {
		loadEntries();
	});

	$effect(() => {
		filterUser;
		filterAction;
		page = 1;
		loadEntries();
	});

	$effect(() => {
		page;
		perPage;
		loadEntries();
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
		if (action === 'login') return 'bg-[var(--green-bg)] text-[var(--green)]';
		if (action === 'login_failed') return 'bg-[var(--red-bg)] text-[var(--red)]';
		if (action.endsWith('_create') || action.endsWith('_deploy')) return 'bg-[var(--accent-bg)] text-[var(--accent)]';
		if (action.endsWith('_delete') || action.endsWith('_stop')) return 'bg-[var(--red-bg)] text-[var(--red)]';
		if (action.endsWith('_update') || action === 'settings_update') return 'bg-[var(--yellow-bg)] text-[var(--yellow)]';
		if (action === 'vuln_scan' || action === 'update_check') return 'bg-[#f3e8ff] text-[#9333ea] dark:bg-[#9333ea20] dark:text-[#c084fc]';
		return 'bg-[var(--bg-hover)] text-[var(--text-secondary)]';
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
				<table class="w-full text-sm">
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
