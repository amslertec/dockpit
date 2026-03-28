<script lang="ts">
	import { widgets, type WidgetLink } from '$lib/stores/widgets';
	import { t } from '$lib/i18n';

	let { widgetId }: { widgetId: string } = $props();

	let editing = $state(false);
	let links = $state<WidgetLink[]>(structuredClone($widgets.find(w => w.id === widgetId)?.links || []));
	let newTitle = $state('');
	let newUrl = $state('');

	function addLink() {
		if (!newTitle.trim() || !newUrl.trim()) return;
		let url = newUrl.trim();
		if (!/^https?:\/\//.test(url)) url = 'https://' + url;
		links = [...links, { title: newTitle.trim(), url }];
		newTitle = '';
		newUrl = '';
	}

	function removeLink(index: number) {
		links = links.filter((_, i) => i !== index);
	}

	function save() {
		widgets.updateWidget(widgetId, { links: structuredClone(links) });
		editing = false;
	}

	function cancel() {
		links = structuredClone($widgets.find(w => w.id === widgetId)?.links || []);
		editing = false;
		newTitle = '';
		newUrl = '';
	}
</script>

<div class="p-4 h-full flex flex-col">
	{#if editing}
		<div class="flex-1 overflow-y-auto space-y-1.5">
			{#each links as link, i}
				<div class="flex items-center gap-2 py-1 border-b border-[var(--border)] last:border-0">
					<div class="flex-1 min-w-0">
						<div class="text-[11px] font-medium text-[var(--text)] truncate">{link.title}</div>
						<div class="text-[10px] text-[var(--text-muted)] truncate">{link.url}</div>
					</div>
					<button
						class="w-5 h-5 flex items-center justify-center rounded text-[var(--text-muted)] hover:text-[var(--red)] transition shrink-0"
						onclick={() => removeLink(i)}
					>
						<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
					</button>
				</div>
			{/each}
			<div class="pt-2 space-y-1.5">
				<input
					type="text"
					class="w-full bg-[var(--bg-2)] border border-[var(--border)] rounded-[var(--radius-sm)] text-[11px] text-[var(--text)] px-2 py-1.5 focus:outline-none focus:border-[var(--accent)]"
					bind:value={newTitle}
					placeholder={$t('dashboard.linkTitle')}
				/>
				<div class="flex gap-1.5">
					<input
						type="text"
						class="flex-1 bg-[var(--bg-2)] border border-[var(--border)] rounded-[var(--radius-sm)] text-[11px] text-[var(--text)] px-2 py-1.5 focus:outline-none focus:border-[var(--accent)]"
						bind:value={newUrl}
						placeholder={$t('dashboard.linkUrl')}
						onkeydown={(e) => { if (e.key === 'Enter') addLink(); }}
					/>
					<button
						class="px-2.5 py-1 text-[10px] font-medium rounded-[var(--radius-sm)] bg-[var(--accent-bg)] text-[var(--accent)] hover:opacity-80 transition shrink-0"
						onclick={addLink}
					>{$t('dashboard.addLink')}</button>
				</div>
			</div>
		</div>
		<div class="flex justify-end gap-2 mt-2 pt-2 border-t border-[var(--border)]">
			<button
				class="px-2.5 py-1 text-[10px] font-medium rounded-[var(--radius-sm)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition"
				onclick={cancel}
			>{$t('common.cancel')}</button>
			<button
				class="px-2.5 py-1 text-[10px] font-medium rounded-[var(--radius-sm)] bg-[var(--accent)] text-white hover:opacity-90 transition"
				onclick={save}
			>{$t('common.save')}</button>
		</div>
	{:else}
		<div class="flex-1 overflow-y-auto">
			{#if links.length === 0}
				<div class="text-xs text-[var(--text-muted)] italic text-center py-4">{$t('dashboard.links')}</div>
			{:else}
				<div class="space-y-1">
					{#each links as link}
						<a
							href={link.url}
							target="_blank"
							rel="noopener noreferrer"
							class="flex items-center gap-2 px-2 py-1.5 rounded-[var(--radius-sm)] text-xs text-[var(--accent)] hover:bg-[var(--accent-bg)] transition no-underline"
						>
							<svg class="w-3 h-3 shrink-0 opacity-60" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
							<span class="truncate">{link.title}</span>
						</a>
					{/each}
				</div>
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
