<script lang="ts">
	import CustomSelect from './CustomSelect.svelte';
	import { t } from '$lib/i18n';

	interface Props {
		total: number;
		page: number;
		perPage: number;
		onchange: (page: number, perPage: number) => void;
	}
	let { total, page, perPage, onchange }: Props = $props();

	const perPageOptions = $derived([
		{ value: 5, label: '5' },
		{ value: 10, label: '10' },
		{ value: 15, label: '15' },
		{ value: 25, label: '25' },
		{ value: 100, label: '100' },
		{ value: 0, label: $t('common.all') }
	]);
	const totalPages = $derived(perPage === 0 ? 1 : Math.ceil(total / perPage));
	const from = $derived(perPage === 0 ? 1 : (page - 1) * perPage + 1);
	const to = $derived(perPage === 0 ? total : Math.min(page * perPage, total));

	function setPage(p: number) {
		if (p < 1 || p > totalPages) return;
		onchange(p, perPage);
	}

	function setPerPage(val: string | number) {
		onchange(1, Number(val));
	}

	function visiblePages(): number[] {
		const pages: number[] = [];
		const start = Math.max(1, page - 2);
		const end = Math.min(totalPages, page + 2);
		for (let i = start; i <= end; i++) pages.push(i);
		return pages;
	}
</script>

{#if total > 0}
	<div class="flex items-center justify-between flex-wrap gap-3 px-4 py-3 border-t border-[var(--border)]">
		<div class="text-[11px] text-[var(--text-muted)]">
			{from}–{to} {$t('common.of')} {total}
		</div>

		<div class="flex items-center gap-3">
			<div class="flex items-center gap-1.5">
				<span class="text-[11px] text-[var(--text-muted)]">{$t('common.perPage')}</span>
				<CustomSelect
					options={perPageOptions}
					value={perPage}
					onchange={setPerPage}
					size="sm"
					class="w-[72px]"
				/>
			</div>

			{#if totalPages > 1}
				<div class="flex items-center gap-0.5">
					<button
						disabled={page <= 1}
						onclick={() => setPage(page - 1)}
						class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed"
					>
						<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
					</button>

					{#if visiblePages()[0] > 1}
						<button onclick={() => setPage(1)} class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-[11px] text-[var(--text-secondary)] hover:text-[var(--text)] hover:bg-[var(--bg-hover)] transition-all duration-200">1</button>
						{#if visiblePages()[0] > 2}
							<span class="text-[11px] text-[var(--text-muted)] px-0.5">...</span>
						{/if}
					{/if}

					{#each visiblePages() as p}
						<button
							onclick={() => setPage(p)}
							class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-[11px] transition-all duration-200
							{p === page ? 'bg-[var(--accent)] text-white font-medium shadow-[var(--shadow-glow)]' : 'text-[var(--text-secondary)] hover:text-[var(--text)] hover:bg-[var(--bg-hover)]'}"
						>{p}</button>
					{/each}

					{#if visiblePages()[visiblePages().length - 1] < totalPages}
						{#if visiblePages()[visiblePages().length - 1] < totalPages - 1}
							<span class="text-[11px] text-[var(--text-muted)] px-0.5">...</span>
						{/if}
						<button onclick={() => setPage(totalPages)} class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-[11px] text-[var(--text-secondary)] hover:text-[var(--text)] hover:bg-[var(--bg-hover)] transition-all duration-200">{totalPages}</button>
					{/if}

					<button
						disabled={page >= totalPages}
						onclick={() => setPage(page + 1)}
						class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed"
					>
						<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
					</button>
				</div>
			{/if}
		</div>
	</div>
{/if}
