<script lang="ts">
	interface Props {
		value: string;
		oninput?: (value: string) => void;
		type?: 'text' | 'password' | 'email' | 'number' | 'url';
		placeholder?: string;
		label?: string;
		error?: string;
		disabled?: boolean;
		required?: boolean;
		id?: string;
		maxlength?: number;
		class?: string;
	}
	let {
		value = $bindable(),
		oninput,
		type = 'text',
		placeholder = '',
		label,
		error,
		disabled = false,
		required = false,
		id,
		maxlength,
		class: className = ''
	}: Props = $props();

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		value = target.value;
		oninput?.(target.value);
	}
</script>

<div class="{className}">
	{#if label}
		<label for={id} class="block text-xs font-medium text-[var(--text-secondary)] mb-1.5">{label}</label>
	{/if}
	<input
		{id}
		{type}
		{value}
		{placeholder}
		{disabled}
		{required}
		{maxlength}
		oninput={handleInput}
		class="w-full bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-3 py-2.5 text-[16px] md:text-sm text-[var(--text)] placeholder:text-[var(--text-muted)]
		focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)]
		transition-all duration-200
		{disabled ? 'opacity-50 cursor-not-allowed' : ''}
		{error ? 'border-[var(--red)] focus:border-[var(--red)] focus:shadow-[0_0_0_3px_var(--red-bg)]' : ''}"
	/>
	{#if error}
		<p class="text-[var(--red)] text-[11px] mt-1">{error}</p>
	{/if}
</div>
