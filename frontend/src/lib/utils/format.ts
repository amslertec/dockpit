import { browser } from '$app/environment';

/** Get the configured timezone from localStorage (set via Settings page) */
function getTimezone(): string {
	if (!browser) return 'UTC';
	return localStorage.getItem('dp_timezone') || Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
}

export function truncateId(id: string, len = 12): string {
	return id.replace('sha256:', '').substring(0, len);
}

export function formatSize(mb: number): string {
	if (mb >= 1000) return `${(mb / 1000).toFixed(1)} GB`;
	return `${mb.toFixed(1)} MB`;
}

/** Format a UNIX timestamp (seconds) → "DD.MM.YYYY HH:MM" in configured timezone */
export function formatDate(timestamp: number): string {
	if (!timestamp) return '—';
	const tz = getTimezone();
	const d = new Date(timestamp * 1000);
	return d.toLocaleDateString('de-CH', { year: 'numeric', month: '2-digit', day: '2-digit', timeZone: tz })
		+ ' ' + d.toLocaleTimeString('de-CH', { hour: '2-digit', minute: '2-digit', timeZone: tz });
}

/** Format a datetime string (UTC from DB, e.g. "2026-03-28 00:09:15") → localized */
export function formatDateTime(dateStr: string | undefined | null): string {
	if (!dateStr) return '—';
	const tz = getTimezone();
	// DB stores UTC without 'Z' suffix, add it
	const d = new Date(dateStr.includes('Z') || dateStr.includes('+') ? dateStr : dateStr + 'Z');
	if (isNaN(d.getTime())) return dateStr;
	return d.toLocaleDateString('de-CH', { year: 'numeric', month: '2-digit', day: '2-digit', timeZone: tz })
		+ ' ' + d.toLocaleTimeString('de-CH', { hour: '2-digit', minute: '2-digit', second: '2-digit', timeZone: tz });
}

/** Format a datetime string → smart format (HH:MM:SS if today, else DD.MM HH:MM) */
export function formatDateTimeSmart(dateStr: string | undefined | null): string {
	if (!dateStr) return '—';
	const tz = getTimezone();
	const d = new Date(dateStr.includes('Z') || dateStr.includes('+') ? dateStr : dateStr + 'Z');
	if (isNaN(d.getTime())) return dateStr;
	const now = new Date();
	const isToday = d.toLocaleDateString('en', { timeZone: tz }) === now.toLocaleDateString('en', { timeZone: tz });
	if (isToday) {
		return d.toLocaleTimeString('de-CH', { hour: '2-digit', minute: '2-digit', second: '2-digit', timeZone: tz });
	}
	return d.toLocaleDateString('de-CH', { day: '2-digit', month: '2-digit', timeZone: tz })
		+ ' ' + d.toLocaleTimeString('de-CH', { hour: '2-digit', minute: '2-digit', timeZone: tz });
}

/** Format relative time (e.g. "2m ago") */
export function formatTimeAgo(dateStr: string | undefined | null): string {
	if (!dateStr) return '—';
	const d = new Date(dateStr.includes('Z') || dateStr.includes('+') ? dateStr : dateStr + 'Z');
	if (isNaN(d.getTime())) return dateStr;
	const diff = Math.floor((Date.now() - d.getTime()) / 1000);
	if (diff < 60) return 'just now';
	if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
	if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
	return `${Math.floor(diff / 86400)}d ago`;
}

export function extractHealth(status: string): string | undefined {
	if (status.includes('(healthy)')) return 'healthy';
	if (status.includes('(unhealthy)')) return 'unhealthy';
	if (status.includes('(health: starting)') || status.includes('health: starting')) return 'starting';
	return undefined;
}

export function formatPorts(ports: { private_port: number; public_port?: number }[]): string {
	const unique = [...new Set(
		ports
			.filter((p) => p.public_port)
			.map((p) => `${p.public_port}:${p.private_port}`)
	)];
	return unique.join(', ') || '—';
}
