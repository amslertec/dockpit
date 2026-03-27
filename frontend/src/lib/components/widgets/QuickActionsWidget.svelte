<script lang="ts">
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import { t } from '$lib/i18n';

	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);
	let running = $state(false);

	async function pruneAll() {
		confirmDlg = { message: $t('widget.confirmCleanup'), action: async () => {
			confirmDlg = null;
			running = true;
			let results: string[] = [];
			for (const env of $environments) {
				const [ir, vr, nr] = await Promise.all([
					api.post<string>(`/env/${env.id}/images/prune`, {}),
					api.post<string>(`/env/${env.id}/volumes/prune`, {}),
					api.post<string>(`/env/${env.id}/networks/prune`, {}),
				]);
				if (ir.success) results.push(`${env.name}: ${ir.data}`);
				if (vr.success) results.push(`${env.name}: ${vr.data}`);
				if (nr.success) results.push(`${env.name}: ${nr.data}`);
			}
			running = false;
			toasts.success($t('widget.cleanupDone', {count: results.length}));
		}};
	}

	async function checkUpdates() {
		running = true;
		toasts.success($t('widget.updateCheckStarted'));
		running = false;
		// Navigate to containers page which auto-checks
		window.location.href = '/containers';
	}
</script>

<div class="p-4 space-y-2">
	<button onclick={pruneAll} disabled={running}
		class="w-full flex items-center gap-3 px-3 py-2.5 bg-1 border border-theme rounded-lg text-left hover:bg-hover hover:border-light transition disabled:opacity-50">
		<div class="w-8 h-8 rounded-lg bg-red-light flex items-center justify-center shrink-0">
			<svg class="w-4 h-4 text-red" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/></svg>
		</div>
		<div>
			<div class="text-xs font-medium text-primary">{$t('widget.systemCleanup')}</div>
			<div class="text-[10px] text-muted">{$t('widget.cleanupDesc')}</div>
		</div>
	</button>

	<button onclick={checkUpdates} disabled={running}
		class="w-full flex items-center gap-3 px-3 py-2.5 bg-1 border border-theme rounded-lg text-left hover:bg-hover hover:border-light transition disabled:opacity-50">
		<div class="w-8 h-8 rounded-lg bg-accent-light flex items-center justify-center shrink-0">
			<svg class="w-4 h-4 text-accent" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
		</div>
		<div>
			<div class="text-xs font-medium text-primary">{$t('widget.checkUpdates')}</div>
			<div class="text-[10px] text-muted">{$t('widget.checkUpdatesDesc')}</div>
		</div>
	</button>

	<a href="/environments" class="w-full flex items-center gap-3 px-3 py-2.5 bg-1 border border-theme rounded-lg text-left hover:bg-hover hover:border-light transition no-underline">
		<div class="w-8 h-8 rounded-lg bg-green-light flex items-center justify-center shrink-0">
			<svg class="w-4 h-4 text-green" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><circle cx="6" cy="6" r="1"/><circle cx="6" cy="18" r="1"/></svg>
		</div>
		<div>
			<div class="text-xs font-medium text-primary">{$t('widget.manageServers')}</div>
			<div class="text-[10px] text-muted">{$t('widget.manageServersDesc')}</div>
		</div>
	</a>
</div>

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
