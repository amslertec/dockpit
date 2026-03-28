<script lang="ts">
	import { widgets } from '$lib/stores/widgets';
	import { t } from '$lib/i18n';

	let { widgetId }: { widgetId: string } = $props();

	let editing = $state(false);
	let text = $state($widgets.find(w => w.id === widgetId)?.content || '');

	function save() {
		widgets.updateWidget(widgetId, { content: text });
		editing = false;
	}
</script>

<div class="p-4 h-full flex flex-col">
	{#if editing}
		<textarea
			class="flex-1 w-full bg-[var(--bg-2)] border border-[var(--border)] rounded-[var(--radius)] text-xs text-[var(--text)] p-2 resize-none focus:outline-none focus:border-[var(--accent)]"
			bind:value={text}
			placeholder={$t('dashboard.note')}
		></textarea>
		<div class="flex justify-end gap-2 mt-2">
			<button
				class="px-2.5 py-1 text-[10px] font-medium rounded-[var(--radius-sm)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition"
				onclick={() => { editing = false; text = $widgets.find(w => w.id === widgetId)?.content || ''; }}
			>{$t('common.cancel')}</button>
			<button
				class="px-2.5 py-1 text-[10px] font-medium rounded-[var(--radius-sm)] bg-[var(--accent)] text-white hover:opacity-90 transition"
				onclick={save}
			>{$t('common.save')}</button>
		</div>
	{:else}
		<div class="flex-1 overflow-y-auto">
			{#if text}
				<div class="text-xs text-[var(--text)] whitespace-pre-wrap leading-relaxed">{text}</div>
			{:else}
				<div class="text-xs text-[var(--text-muted)] italic">{$t('dashboard.note')}</div>
			{/if}
		</div>
		<div class="flex justify-end mt-2">
			<button
				class="w-6 h-6 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--text-muted)] hover:text-[var(--accent)] hover:bg-[var(--accent-bg)] transition"
				onclick={() => editing = true}
				title={$t('common.edit')}
			>
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
			</button>
		</div>
	{/if}
</div>
