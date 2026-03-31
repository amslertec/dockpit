<script lang="ts">
	import { onMount } from 'svelte';
	import { t } from '$lib/i18n';
	import { api } from '$lib/api/client';
	import type { NotificationInfo } from '$lib/api/types';
	import { formatTimeAgo } from '$lib/utils/format';


	interface Props {
		onclose: () => void;
		onchange?: () => void;
	}
	let { onclose, onchange }: Props = $props();

	let notifications = $state<NotificationInfo[]>([]);
	let loading = $state(true);

	async function fetchNotifications() {
		const res = await api.get<NotificationInfo[]>('/notifications');
		if (res.success && res.data) {
			notifications = res.data;
		}
		loading = false;
	}

	async function markAsRead(id: number) {
		await api.post(`/notifications/${id}/read`, {});
		notifications = notifications.map((n) => (n.id === id ? { ...n, read: true } : n));
		onchange?.();
	}

	async function markAllAsRead() {
		await api.post('/notifications/read-all', {});
		notifications = notifications.map((n) => ({ ...n, read: true }));
		onchange?.();
	}

	async function deleteNotification(id: number) {
		await api.del(`/notifications/${id}`);
		notifications = notifications.filter((n) => n.id !== id);
	}

	function handleWindowClick(e: MouseEvent) {
		const panel = document.getElementById('notification-panel');
		if (panel && !panel.contains(e.target as Node)) {
			onclose();
		}
	}

	onMount(() => {
		fetchNotifications();
	});
</script>

<svelte:window onclick={handleWindowClick} />

<div
	id="notification-panel"
	class="fixed right-4 top-[56px] z-[9999] w-[380px] max-h-[400px] bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] flex flex-col overflow-hidden notification-enter"
	onclick={(e) => e.stopPropagation()}
	role="dialog"
	aria-label={$t('notifications.title')}
>
	<!-- Header -->
	<div class="flex items-center justify-between px-4 py-3 border-b border-[var(--border)]">
		<span class="text-sm font-semibold text-[var(--text)]">{$t('notifications.title')}</span>
		<button
			class="text-[11px] font-medium text-[var(--accent)] hover:text-[var(--accent-hover)] transition-colors duration-150 cursor-pointer"
			onclick={markAllAsRead}
		>{$t('notifications.markAllRead')}</button>
	</div>

	<!-- List -->
	<div class="overflow-y-auto flex-1">
		{#if loading}
			<div class="flex items-center justify-center py-8 text-xs text-[var(--text-muted)]">
				{$t('common.loading')}
			</div>
		{:else if notifications.length === 0}
			<div class="flex flex-col items-center justify-center py-10 gap-2">
				<svg class="w-8 h-8 text-[var(--text-muted)] opacity-40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9" /><path d="M13.73 21a2 2 0 01-3.46 0" />
				</svg>
				<span class="text-sm font-medium text-[var(--text-muted)]">{$t('notifications.empty')}</span>
				<span class="text-xs text-[var(--text-muted)] opacity-70">{$t('notifications.emptyDesc')}</span>
			</div>
		{:else}
			{#each notifications as notif (notif.id)}
				<div
					class="flex items-start gap-3 px-4 py-3 border-b border-[var(--border)] hover:bg-[var(--bg-hover)] transition-colors duration-150 cursor-pointer {notif.read ? 'opacity-60' : ''}"
					onclick={() => markAsRead(notif.id)}
					role="button"
					tabindex="0"
					onkeydown={(e) => { if (e.key === 'Enter') markAsRead(notif.id); }}
				>
					<!-- Icon -->
					<div class="shrink-0 mt-0.5">
						{#if notif.type === 'success'}
							<div class="w-7 h-7 rounded-full bg-[var(--green)]/15 flex items-center justify-center">
								<svg class="w-3.5 h-3.5 text-[var(--green)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12" /></svg>
							</div>
						{:else if notif.type === 'error'}
							<div class="w-7 h-7 rounded-full bg-[var(--red)]/15 flex items-center justify-center">
								<svg class="w-3.5 h-3.5 text-[var(--red)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
							</div>
						{:else}
							<div class="w-7 h-7 rounded-full bg-[var(--accent)]/15 flex items-center justify-center">
								<svg class="w-3.5 h-3.5 text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10" /><line x1="12" y1="16" x2="12" y2="12" /><line x1="12" y1="8" x2="12.01" y2="8" /></svg>
							</div>
						{/if}
					</div>

					<!-- Content -->
					<div class="flex-1 min-w-0">
						<div class="flex items-center gap-2">
							<span class="text-xs font-semibold text-[var(--text)] truncate">{notif.title}</span>
							{#if !notif.read}
								<span class="w-2 h-2 rounded-full bg-[var(--accent)] shrink-0"></span>
							{/if}
						</div>
						<p class="text-[11px] text-[var(--text-secondary)] mt-0.5 line-clamp-2">{notif.message}</p>
						<span class="text-[10px] text-[var(--text-muted)] mt-1 block">{formatTimeAgo(notif.created_at)}</span>
					</div>

					<!-- Delete button -->
					<button
						class="shrink-0 mt-0.5 w-6 h-6 flex items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:text-[var(--red)] hover:bg-[var(--bg-hover)] transition-all"
						aria-label="Delete"
						onclick={(e) => { e.stopPropagation(); deleteNotification(notif.id); }}
					>
						<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
					</button>
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.notification-enter {
		animation: notification-slide 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}
	@keyframes notification-slide {
		from {
			opacity: 0;
			transform: translateY(-8px) scale(0.96);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}
</style>
