export function truncateId(id: string, len = 12): string {
	return id.replace('sha256:', '').substring(0, len);
}

export function formatSize(mb: number): string {
	if (mb >= 1000) return `${(mb / 1000).toFixed(1)} GB`;
	return `${mb.toFixed(1)} MB`;
}

export function formatDate(timestamp: number): string {
	if (!timestamp) return '—';
	const d = new Date(timestamp * 1000);
	return d.toLocaleDateString('de-CH', { year: 'numeric', month: '2-digit', day: '2-digit' })
		+ ' ' + d.toLocaleTimeString('de-CH', { hour: '2-digit', minute: '2-digit' });
}

export function extractHealth(status: string): string | undefined {
	if (status.includes('(healthy)')) return 'healthy';
	if (status.includes('(unhealthy)')) return 'unhealthy';
	if (status.includes('(health: starting)') || status.includes('health: starting')) return 'starting';
	return undefined;
}

export function formatPorts(ports: { private_port: number; public_port?: number }[]): string {
	return (
		ports
			.filter((p) => p.public_port)
			.map((p) => `${p.public_port}:${p.private_port}`)
			.join(', ') || '—'
	);
}
