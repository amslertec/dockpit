<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { t } from '$lib/i18n';

	interface Option {
		value: string | number;
		label: string;
	}
	interface Props {
		options: Option[];
		value: string | number;
		onchange: (value: string | number) => void;
		placeholder?: string;
		disabled?: boolean;
		size?: 'sm' | 'md';
		class?: string;
	}
	let {
		options,
		value,
		onchange,
		placeholder = '',
		disabled = false,
		size = 'md',
		class: className = ''
	}: Props = $props();

	let open = $state(false);
	let highlightIndex = $state(-1);
	let triggerEl: HTMLButtonElement | undefined = $state();
	let dropdownEl: HTMLDivElement | undefined = $state();
	let dropdownStyle = $state('');

	const selectedLabel = $derived(options.find(o => o.value === value)?.label || placeholder || $t('common.select'));

	// Move dropdown to body when it opens (escape any stacking context)
	$effect(() => {
		if (open && dropdownEl) {
			document.body.appendChild(dropdownEl);
		}
	});

	onDestroy(() => {
		// Clean up portal element from body
		if (dropdownEl && dropdownEl.parentElement === document.body) {
			dropdownEl.remove();
		}
	});

	function positionDropdown() {
		if (!triggerEl) return;
		const rect = triggerEl.getBoundingClientRect();
		const spaceBelow = window.innerHeight - rect.bottom;
		const dropdownHeight = Math.min(options.length * 40 + 16, 240);
		const openAbove = spaceBelow < dropdownHeight && rect.top > dropdownHeight;
		const w = Math.max(rect.width, 180);

		// Check if dropdown would overflow right edge
		let left = rect.left;
		if (left + w > window.innerWidth - 8) {
			left = rect.right - w;
			if (left < 8) left = 8;
		}

		if (openAbove) {
			dropdownStyle = `position:fixed; left:${left}px; bottom:${window.innerHeight - rect.top + 4}px; width:${w}px; z-index:99999;`;
		} else {
			dropdownStyle = `position:fixed; left:${left}px; top:${rect.bottom + 4}px; width:${w}px; z-index:99999;`;
		}
	}

	function toggle(e: MouseEvent) {
		e.stopPropagation();
		if (disabled) return;
		if (open) {
			open = false;
			return;
		}
		highlightIndex = options.findIndex(o => o.value === value);
		positionDropdown();
		open = true;
	}

	function select(opt: Option) {
		onchange(opt.value);
		open = false;
		triggerEl?.focus();
	}

	function onkeydown(e: KeyboardEvent) {
		if (!open) {
			if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				highlightIndex = options.findIndex(o => o.value === value);
				positionDropdown();
				open = true;
				return;
			}
			return;
		}
		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				highlightIndex = (highlightIndex + 1) % options.length;
				scrollToHighlighted();
				break;
			case 'ArrowUp':
				e.preventDefault();
				highlightIndex = (highlightIndex - 1 + options.length) % options.length;
				scrollToHighlighted();
				break;
			case 'Enter':
			case ' ':
				e.preventDefault();
				if (highlightIndex >= 0 && highlightIndex < options.length) select(options[highlightIndex]);
				break;
			case 'Escape':
				e.preventDefault();
				open = false;
				triggerEl?.focus();
				break;
		}
	}

	function scrollToHighlighted() {
		requestAnimationFrame(() => {
			dropdownEl?.querySelector(`[data-index="${highlightIndex}"]`)?.scrollIntoView({ block: 'nearest' });
		});
	}

	const sizeClasses: Record<string, string> = {
		sm: 'px-2.5 py-1.5 text-[11px]',
		md: 'px-3 py-2.5 text-xs'
	};
</script>

<svelte:window onclick={() => { open = false; }} />

<div class="relative {className}">
	<button
		bind:this={triggerEl}
		type="button"
		{disabled}
		class="w-full flex items-center justify-between gap-2 bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] {sizeClasses[size]} text-[var(--text)] transition-all duration-200
		{open ? 'border-[var(--input-focus)] shadow-[0_0_0_3px_var(--input-focus-ring)]' : 'hover:border-[var(--border-light)]'}
		{disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}"
		onclick={toggle}
		onkeydown={onkeydown}
		aria-expanded={open}
		aria-haspopup="listbox"
	>
		<span class="truncate {value === '' || value === undefined ? 'text-[var(--text-muted)]' : ''}">{selectedLabel}</span>
		<svg class="w-3 h-3 text-[var(--text-muted)] shrink-0 transition-transform duration-200 {open ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
	</button>
</div>

{#if open}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		bind:this={dropdownEl}
		class="select-dropdown-enter"
		style={dropdownStyle}
		onclick={(e) => e.stopPropagation()}
	>
		<div
			class="py-1.5 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] max-h-[240px] overflow-y-auto"
			role="listbox"
		>
			{#each options as opt, i}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					data-index={i}
					class="flex items-center justify-between px-3 py-2 {sizeClasses[size]} cursor-pointer transition-all duration-100
					{opt.value === value ? 'text-[var(--accent)] bg-[var(--accent-bg)]' : 'text-[var(--text-secondary)]'}
					{i === highlightIndex ? 'bg-[var(--bg-hover)] text-[var(--text)]' : ''}
					hover:bg-[var(--bg-hover)] hover:text-[var(--text)]"
					role="option"
					aria-selected={opt.value === value}
					onclick={() => select(opt)}
				>
					<span class="truncate">{opt.label}</span>
					{#if opt.value === value}
						<svg class="w-3.5 h-3.5 shrink-0 text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
					{/if}
				</div>
			{/each}
		</div>
	</div>
{/if}

<style>
	.select-dropdown-enter {
		animation: select-open 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes select-open {
		from { opacity: 0; transform: translateY(-6px) scale(0.97); }
		to { opacity: 1; transform: translateY(0) scale(1); }
	}
</style>
