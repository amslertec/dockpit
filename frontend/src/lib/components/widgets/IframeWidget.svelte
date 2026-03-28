<script lang="ts">
	import { widgets } from '$lib/stores/widgets';
	import { t } from '$lib/i18n';

	let { widgetId }: { widgetId: string } = $props();

	let url = $state($widgets.find(w => w.id === widgetId)?.iframeUrl || '');
	let inputUrl = $state('');

	function save() {
		let u = inputUrl.trim();
		if (!u) return;
		if (!/^https?:\/\//.test(u)) u = 'https://' + u;
		url = u;
		widgets.updateWidget(widgetId, { iframeUrl: u });
		inputUrl = '';
	}
</script>

<div class="h-full flex flex-col">
	{#if url}
		<iframe
			src={url}
			sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
			class="flex-1 w-full border-0"
			title="Embed"
		></iframe>
	{:else}
		<div class="flex-1 flex flex-col items-center justify-center p-4 gap-3">
			<svg class="w-8 h-8 text-[var(--text-muted)] opacity-40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
			<div class="flex gap-1.5 w-full max-w-[280px]">
				<input
					type="text"
					class="flex-1 bg-[var(--bg-2)] border border-[var(--border)] rounded-[var(--radius-sm)] text-[11px] text-[var(--text)] px-2 py-1.5 focus:outline-none focus:border-[var(--accent)]"
					bind:value={inputUrl}
					placeholder={$t('dashboard.enterUrl')}
					onkeydown={(e) => { if (e.key === 'Enter') save(); }}
				/>
				<button
					class="px-3 py-1.5 text-[10px] font-medium rounded-[var(--radius-sm)] bg-[var(--accent)] text-white hover:opacity-90 transition shrink-0"
					onclick={save}
				>{$t('common.save')}</button>
			</div>
		</div>
	{/if}
</div>
