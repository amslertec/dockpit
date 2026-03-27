<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'primary' | 'danger' | 'warning' | 'success' | 'secondary' | 'ghost' | 'purple';
		size?: 'sm' | 'md' | 'lg';
		disabled?: boolean;
		loading?: boolean;
		type?: 'button' | 'submit' | 'reset';
		title?: string;
		onclick?: (e: MouseEvent) => void;
		children: Snippet;
		class?: string;
	}
	let {
		variant = 'primary',
		size = 'md',
		disabled = false,
		loading = false,
		type = 'button',
		title,
		onclick,
		children,
		class: className = ''
	}: Props = $props();
</script>

<button
	{type}
	{title}
	disabled={disabled || loading}
	class="btn btn-{variant} btn-{size} {disabled || loading ? 'btn-disabled' : ''} {className}"
	onclick={onclick}
>
	{#if loading}
		<div class="w-3.5 h-3.5 border-2 border-current/30 border-t-current rounded-full animate-spin shrink-0"></div>
	{/if}
	{@render children()}
</button>

<style>
	.btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		font-weight: 600;
		cursor: pointer;
		user-select: none;
		transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
		position: relative;
		overflow: hidden;
		letter-spacing: 0.01em;
	}

	/* Sizes */
	.btn-sm { padding: 6px 12px; font-size: 11px; border-radius: var(--radius-sm); }
	.btn-md { padding: 8px 18px; font-size: 12px; border-radius: var(--radius-md); }
	.btn-lg { padding: 10px 24px; font-size: 13px; border-radius: var(--radius-md); }

	/* Primary — gradient accent */
	.btn-primary {
		background: var(--gradient-primary);
		color: white;
		box-shadow: var(--shadow-sm), 0 0 20px rgba(108, 92, 231, 0.15);
		border: 1px solid rgba(255, 255, 255, 0.1);
	}
	.btn-primary:hover {
		box-shadow: var(--shadow-md), 0 0 30px rgba(108, 92, 231, 0.3);
		transform: translateY(-1px);
		filter: brightness(1.1);
	}

	/* Danger — gradient red */
	.btn-danger {
		background: var(--gradient-danger);
		color: white;
		box-shadow: var(--shadow-sm);
		border: 1px solid rgba(255, 255, 255, 0.08);
	}
	.btn-danger:hover {
		box-shadow: var(--shadow-md), var(--shadow-glow-red);
		transform: translateY(-1px);
		filter: brightness(1.1);
	}

	/* Success — gradient green */
	.btn-success {
		background: var(--gradient-success);
		color: white;
		box-shadow: var(--shadow-sm);
		border: 1px solid rgba(255, 255, 255, 0.08);
	}
	.btn-success:hover {
		box-shadow: var(--shadow-md), var(--shadow-glow-green);
		transform: translateY(-1px);
		filter: brightness(1.1);
	}

	/* Warning — solid yellow */
	.btn-warning {
		background: var(--yellow);
		color: #1a1d2e;
		box-shadow: var(--shadow-sm);
		border: 1px solid rgba(255, 255, 255, 0.1);
	}
	.btn-warning:hover {
		box-shadow: var(--shadow-md), 0 0 20px rgba(255, 190, 46, 0.2);
		transform: translateY(-1px);
		filter: brightness(1.1);
	}

	/* Purple — solid purple */
	.btn-purple {
		background: var(--purple);
		color: white;
		box-shadow: var(--shadow-sm);
		border: 1px solid rgba(255, 255, 255, 0.08);
	}
	.btn-purple:hover {
		box-shadow: var(--shadow-md), 0 0 20px rgba(191, 138, 255, 0.2);
		transform: translateY(-1px);
		filter: brightness(1.1);
	}

	/* Secondary — glass border */
	.btn-secondary {
		background: transparent;
		color: var(--text-secondary);
		border: 1px solid var(--border);
	}
	.btn-secondary:hover {
		color: var(--text);
		border-color: var(--border-light);
		background: var(--bg-hover);
	}

	/* Ghost */
	.btn-ghost {
		background: transparent;
		color: var(--text-secondary);
		border: 1px solid transparent;
	}
	.btn-ghost:hover {
		color: var(--text);
		background: var(--bg-hover);
	}

	/* Disabled */
	.btn-disabled {
		opacity: 0.5;
		cursor: not-allowed;
		pointer-events: none;
		transform: none !important;
	}
</style>
