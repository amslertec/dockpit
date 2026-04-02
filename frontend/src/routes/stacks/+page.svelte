<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { selectedEnv, environments } from '$lib/stores/environment';
	import { toasts } from '$lib/stores/toast';
	import { t } from '$lib/i18n';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import Pagination from '$lib/components/ui/Pagination.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import YAML from 'yaml';
	import CustomCheckbox from '$lib/components/ui/CustomCheckbox.svelte';
	import { canManageDocker, canEditContainers, canDoAction, canSeePage } from '$lib/stores/auth';
	import type { StackInfo, StackFile, StackTemplate } from '$lib/api/types';

	let stacks = $state<StackInfo[]>([]);
	let loading = $state(true);
	let search = $state('');
	let showCreate = $state(false);
	let page = $state(1);
	let perPage = $state(10);
	let confirmDlg = $state<{ message: string; action: () => void } | null>(null);

	// Template state
	let templates = $state<StackTemplate[]>([]);
	let showTemplates = $state(false);
	let showSaveTemplate = $state(false);
	let saveTemplateName = $state('');
	let saveTemplateDesc = $state('');
	let showCreateTemplate = $state(false);
	let newTemplateName = $state('');
	let newTemplateDesc = $state('');
	let newTemplateCategory = $state('custom');
	let newTemplateCompose = $state('');
	let newTemplateEnv = $state('');
	let newTemplateIcon = $state('📦');
	const iconOptions = ['📦', '🐳', '🔀', '📊', '📈', '🐘', '🔴', '📝', '☁️', '🛡️', '🗼', '🌐', '⚙️', '🔧', '🗄️', '💾', '🔒', '📡', '🎯', '🚀'];

	// Create modal state
	let newName = $state('');
	let createTab = $state(0);
	let saving = $state(false);
	let yamlError = $state('');

	interface Tab { name: string; content: string; removable: boolean; }
	let tabs = $state<Tab[]>([]);

	function resetCreate() {
		newName = ''; createTab = 0; yamlError = '';
		tabs = [
			{ name: 'docker-compose.yml', content: `services:\n  app:\n    image: nginx:latest\n    ports:\n      - "8080:80"\n    restart: unless-stopped\n`, removable: false },
			{ name: '.env', content: '', removable: false }
		];
	}

	// Docker Run converter state
	let showRunConvert = $state(false);
	let dockerRunInput = $state('');
	let convertedYaml = $state('');
	let convertError = $state('');
	let convertedName = $state('');

	let sortKey = $state<string>('name');
	let sortAsc = $state(true);

	function toggleSort(key: string) {
		if (sortKey === key) { sortAsc = !sortAsc; }
		else { sortKey = key; sortAsc = false; }
	}

	function sortIndicator(key: string): string {
		if (sortKey !== key) return '';
		return sortAsc ? ' ▲' : ' ▼';
	}

	const filtered = $derived(
		stacks
			.filter((s) => s.name.toLowerCase().includes(search.toLowerCase()))
			.sort((a, b) => {
				const av = (a as any)[sortKey];
				const bv = (b as any)[sortKey];
				if (typeof av === 'number' && typeof bv === 'number') return sortAsc ? av - bv : bv - av;
				return sortAsc ? String(av ?? '').localeCompare(String(bv ?? '')) : String(bv ?? '').localeCompare(String(av ?? ''));
			})
	);
	const paged = $derived(perPage === 0 ? filtered : filtered.slice((page - 1) * perPage, page * perPage));

	// Stack migration
	let migrateStack = $state<string | null>(null);
	let migrateTarget = $state('');
	let migrateStopSource = $state(true);
	let migrateDeploy = $state(true);
	let migrateDropdown = $state(false);
	let migrating = $state(false);
	const migrateTargets = $derived($environments.filter(e => e.id !== $selectedEnv).map(e => ({ value: e.id, label: e.name })));

	async function doMigrateStack() {
		if (!migrateStack || !migrateTarget) return;
		migrating = true;
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${migrateStack}/migrate`, {
			target_env_id: migrateTarget, stop_source: migrateStopSource, deploy: migrateDeploy,
		});
		migrating = false;
		if (r.success) { toasts.success(r.data || $t('stacks.migrated')); migrateStack = null; migrateTarget = ''; load(); }
		else toasts.error(r.error || $t('common.error'));
	}

	// Stats
	const runningCount = $derived(stacks.filter(s => s.status === 'running').length);
	const stoppedCount = $derived(stacks.filter(s => s.status === 'stopped').length);

	$effect(() => {
		if (!$canSeePage('page.stacks')) goto('/profile');
	});

	onMount(() => { resetCreate(); load(); loadTemplates(); });
	$effect(() => { $selectedEnv; load(); });
	$effect(() => { search; page = 1; }); // Reset page on search

	async function load() {
		if (!$selectedEnv) return;
		loading = true;
		const r = await api.get<StackInfo[]>(`/env/${$selectedEnv}/stacks`);
		if (r.success) stacks = r.data || [];
		loading = false;
	}

	function validateYaml(c: string): string {
		if (!c.trim()) return $t('stacks.yamlEmpty');
		if (c.includes('\t')) return $t('stacks.yamlTabs');
		try { YAML.parse(c); return ''; } catch (e) { return (e as Error).message; }
	}

	function onInput() { if (createTab === 0) yamlError = validateYaml(tabs[0].content); }

	function addFile() {
		const f = prompt('Filename (e.g. prometheus.yml):');
		if (!f?.trim()) return;
		if (tabs.some((t) => t.name === f.trim())) { toasts.error($t('stacks.alreadyExists')); return; }
		tabs = [...tabs, { name: f.trim(), content: '', removable: true }];
		createTab = tabs.length - 1;
	}

	function removeTab(idx: number) {
		tabs = tabs.filter((_, i) => i !== idx);
		if (createTab >= tabs.length) createTab = tabs.length - 1;
	}

	async function create() {
		if (!newName.trim()) { toasts.error($t('stacks.stackNameRequired')); return; }
		const err = validateYaml(tabs[0].content);
		if (err) { yamlError = err; createTab = 0; toasts.error($t('stacks.yamlError')); return; }
		const extra: StackFile[] = tabs.slice(2).filter((t) => t.content.trim()).map((t) => ({ name: t.name, content: t.content }));
		saving = true;
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks`, {
			name: newName.trim(), compose_content: tabs[0].content,
			env_content: tabs[1].content.trim() || null,
			extra_files: extra.length > 0 ? extra : null
		});
		saving = false;
		if (r.success) { showCreate = false; toasts.success($t('stacks.created', { name: newName })); resetCreate(); load(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function deploy(name: string) {
		toasts.success($t('stacks.stackStarting'));
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${name}/deploy`, {});
		if (r.success) { toasts.success($t('stacks.started')); setTimeout(load, 1500); } else toasts.error(r.error || $t('common.error'));
	}

	async function stop(name: string) {
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks/${name}/stop`, {});
		if (r.success) { toasts.success($t('containers.stop')); setTimeout(load, 1000); } else toasts.error(r.error || $t('common.error'));
	}

	function remove(name: string) {
		confirmDlg = { message: $t('stacks.confirmDelete', { name }), action: async () => {
			confirmDlg = null;
			await api.post<string>(`/env/${$selectedEnv}/stacks/${name}/stop`, {});
			const r = await api.del<string>(`/env/${$selectedEnv}/stacks/${name}`);
			if (r.success) { toasts.success($t('common.delete')); load(); } else toasts.error(r.error || $t('common.error'));
		}};
	}

	async function loadTemplates() {
		const r = await api.get<StackTemplate[]>('/templates');
		if (r.success) templates = r.data || [];
	}

	function useTemplate(tpl: StackTemplate) {
		showTemplates = false;
		resetCreate();
		tabs[0].content = tpl.compose_content;
		tabs[1].content = tpl.env_content || '';
		newName = tpl.name.toLowerCase().replace(/[^a-z0-9-]/g, '-').replace(/-+/g, '-');
		showCreate = true;
	}

	async function deleteTemplate(id: string) {
		const r = await api.del<string>(`/templates/${id}`);
		if (r.success) { toasts.success($t('templates.deleted')); loadTemplates(); }
		else toasts.error(r.error || $t('common.error'));
	}

	async function saveCustomTemplate() {
		if (!newTemplateName.trim()) return;
		const r = await api.post<StackTemplate>('/templates', {
			name: newTemplateName.trim(),
			description: newTemplateDesc.trim() || null,
			category: newTemplateCategory || 'custom',
			compose_content: newTemplateCompose || `services:\n  app:\n    image: nginx:latest\n    ports:\n      - "8080:80"\n`,
			env_content: newTemplateEnv.trim() || null,
			icon: newTemplateIcon
		});
		if (r.success) {
			toasts.success($t('templates.saved'));
			newTemplateName = ''; newTemplateDesc = ''; newTemplateCompose = ''; newTemplateEnv = ''; newTemplateIcon = '📦';
			showCreateTemplate = false;
			loadTemplates();
		} else toasts.error(r.error || $t('common.error'));
	}

	async function saveAsTemplate() {
		if (!saveTemplateName.trim()) return;
		const r = await api.post<StackTemplate>('/templates', {
			name: saveTemplateName.trim(),
			description: saveTemplateDesc.trim() || null,
			category: 'custom',
			compose_content: tabs[0].content,
			env_content: tabs[1].content.trim() || null,
			icon: '📦'
		});
		if (r.success) {
			toasts.success($t('templates.saved'));
			saveTemplateName = ''; saveTemplateDesc = '';
			showSaveTemplate = false;
			loadTemplates();
		} else toasts.error(r.error || $t('common.error'));
	}

	function parseDockerRun(input: string): { name: string; yaml: string } | { error: string } {
		const cmd = input.trim().replace(/\\\n/g, ' ').replace(/\s+/g, ' ');
		if (!cmd.startsWith('docker run')) return { error: 'Command must start with "docker run"' };

		const parts: string[] = [];
		let current = '';
		let inQuote = '';
		for (const ch of cmd) {
			if ((ch === '"' || ch === "'") && !inQuote) { inQuote = ch; continue; }
			if (ch === inQuote) { inQuote = ''; continue; }
			if (ch === ' ' && !inQuote) { if (current) parts.push(current); current = ''; continue; }
			current += ch;
		}
		if (current) parts.push(current);

		// Skip "docker" and "run"
		let i = 0;
		while (i < parts.length && parts[i] !== 'run') i++;
		i++; // skip "run"

		let name = '';
		let image = '';
		const ports: string[] = [];
		const volumes: string[] = [];
		const envs: string[] = [];
		const envFiles: string[] = [];
		const labels: string[] = [];
		const networks: string[] = [];
		const capAdd: string[] = [];
		const capDrop: string[] = [];
		const devices: string[] = [];
		const tmpfs: string[] = [];
		const dns: string[] = [];
		const extraHosts: string[] = [];
		const securityOpt: string[] = [];
		const ulimits: string[] = [];
		const sysctls: string[] = [];
		const logOpts: string[] = [];
		let restart = '';
		let workdir = '';
		let user = '';
		let hostname = '';
		let domainname = '';
		let entrypoint = '';
		let memory = '';
		let cpus = '';
		let shmSize = '';
		let pid = '';
		let ipc = '';
		let stopSignal = '';
		let stopGrace = '';
		let logDriver = '';
		let healthCmd = '';
		let healthInterval = '';
		let healthRetries = '';
		let healthTimeout = '';
		let platform = '';
		let privileged = false;
		let readOnly = false;
		let init = false;
		let tty = false;
		let stdinOpen = false;
		const cmdArgs: string[] = [];
		let imageFound = false;

		while (i < parts.length) {
			const p = parts[i];
			if (imageFound) { cmdArgs.push(p); i++; continue; }

			// Flags with values (--flag value or --flag=value)
			const getVal = (): string => {
				if (p.includes('=')) return p.split('=').slice(1).join('=');
				i++;
				return parts[i] || '';
			};

			if (p === '--name') { name = getVal(); }
			else if (p === '-p' || p === '--publish') { ports.push(getVal()); }
			else if (p === '-v' || p === '--volume') { volumes.push(getVal()); }
			else if (p === '-e' || p === '--env') { envs.push(getVal()); }
			else if (p === '--env-file') { envFiles.push(getVal()); }
			else if (p === '--restart') { restart = getVal(); }
			else if (p === '--network' || p === '--net') { networks.push(getVal()); }
			else if (p === '-l' || p === '--label') { labels.push(getVal()); }
			else if (p === '-w' || p === '--workdir') { workdir = getVal(); }
			else if (p === '-u' || p === '--user') { user = getVal(); }
			else if (p === '-h' || p === '--hostname') { hostname = getVal(); }
			else if (p === '--domainname') { domainname = getVal(); }
			else if (p === '--entrypoint') { entrypoint = getVal(); }
			else if (p === '-m' || p === '--memory') { memory = getVal(); }
			else if (p === '--cpus') { cpus = getVal(); }
			else if (p === '--shm-size') { shmSize = getVal(); }
			else if (p === '--pid') { pid = getVal(); }
			else if (p === '--ipc') { ipc = getVal(); }
			else if (p === '--cap-add') { capAdd.push(getVal()); }
			else if (p === '--cap-drop') { capDrop.push(getVal()); }
			else if (p === '--device') { devices.push(getVal()); }
			else if (p === '--tmpfs') { tmpfs.push(getVal()); }
			else if (p === '--dns') { dns.push(getVal()); }
			else if (p === '--add-host' || p === '--extra-hosts') { extraHosts.push(getVal()); }
			else if (p === '--security-opt') { securityOpt.push(getVal()); }
			else if (p === '--ulimit') { ulimits.push(getVal()); }
			else if (p === '--sysctl') { sysctls.push(getVal()); }
			else if (p === '--log-driver') { logDriver = getVal(); }
			else if (p === '--log-opt') { logOpts.push(getVal()); }
			else if (p === '--stop-signal') { stopSignal = getVal(); }
			else if (p === '--stop-timeout') { stopGrace = getVal() + 's'; }
			else if (p === '--stop-grace-period') { stopGrace = getVal(); }
			else if (p === '--health-cmd') { healthCmd = getVal(); }
			else if (p === '--health-interval') { healthInterval = getVal(); }
			else if (p === '--health-retries') { healthRetries = getVal(); }
			else if (p === '--health-timeout') { healthTimeout = getVal(); }
			else if (p === '--platform') { platform = getVal(); }
			else if (p === '--gpus') { getVal(); }
			else if (p.startsWith('--name=')) { name = p.split('=').slice(1).join('='); }
			else if (p.startsWith('-p=') || p.startsWith('--publish=')) { ports.push(p.split('=').slice(1).join('=')); }
			else if (p.startsWith('-e=') || p.startsWith('--env=')) { envs.push(p.split('=').slice(1).join('=')); }
			// Boolean flags
			else if (p === '-d' || p === '--detach') { /* skip */ }
			else if (p === '--privileged') { privileged = true; }
			else if (p === '--read-only') { readOnly = true; }
			else if (p === '--init') { init = true; }
			else if (p === '-t' || p === '--tty') { tty = true; }
			else if (p === '-i' || p === '--interactive') { stdinOpen = true; }
			else if (p === '--rm') { /* skip - not relevant for compose */ }
			// Combined short flags like -dit
			else if (p.startsWith('-') && !p.startsWith('--') && p.length > 2) {
				for (const ch of p.slice(1)) {
					if (ch === 'd') { /* detach */ }
					else if (ch === 't') { tty = true; }
					else if (ch === 'i') { stdinOpen = true; }
				}
			}
			// Unknown --flag with value: skip flag + value
			else if (p.startsWith('--') && i + 1 < parts.length && !parts[i + 1].startsWith('-')) {
				i++; // skip the value of the unknown flag
			}
			// Image (first non-flag argument)
			else if (!p.startsWith('-')) {
				image = p;
				imageFound = true;
			}
			i++;
		}

		if (!image) return { error: 'No image specified' };

		const svcName = name || image.split(':')[0].split('/').pop() || 'app';

		let yaml = 'services:\n';
		yaml += `  ${svcName}:\n`;
		yaml += `    image: ${image}\n`;
		if (name) yaml += `    container_name: ${name}\n`;
		if (restart) yaml += `    restart: ${restart}\n`;
		if (hostname) yaml += `    hostname: ${hostname}\n`;
		if (domainname) yaml += `    domainname: ${domainname}\n`;
		if (user) yaml += `    user: "${user}"\n`;
		if (workdir) yaml += `    working_dir: ${workdir}\n`;
		if (entrypoint) yaml += `    entrypoint: ${entrypoint}\n`;
		if (privileged) yaml += `    privileged: true\n`;
		if (readOnly) yaml += `    read_only: true\n`;
		if (init) yaml += `    init: true\n`;
		if (tty) yaml += `    tty: true\n`;
		if (stdinOpen) yaml += `    stdin_open: true\n`;
		if (pid) yaml += `    pid: "${pid}"\n`;
		if (ipc) yaml += `    ipc: "${ipc}"\n`;
		if (shmSize) yaml += `    shm_size: "${shmSize}"\n`;
		if (stopSignal) yaml += `    stop_signal: ${stopSignal}\n`;
		if (stopGrace) yaml += `    stop_grace_period: ${stopGrace}\n`;
		if (platform) yaml += `    platform: ${platform}\n`;
		if (memory || cpus) {
			if (memory) yaml += `    mem_limit: ${memory}\n`;
			if (cpus) yaml += `    cpus: ${cpus}\n`;
		}
		if (ports.length > 0) {
			yaml += '    ports:\n';
			for (const port of ports) yaml += `      - "${port}"\n`;
		}
		if (volumes.length > 0) {
			yaml += '    volumes:\n';
			for (const v of volumes) yaml += `      - ${v}\n`;
		}
		if (envs.length > 0) {
			yaml += '    environment:\n';
			for (const e of envs) yaml += `      - ${e}\n`;
		}
		if (envFiles.length > 0) {
			yaml += '    env_file:\n';
			for (const f of envFiles) yaml += `      - ${f}\n`;
		}
		if (labels.length > 0) {
			yaml += '    labels:\n';
			for (const l of labels) yaml += `      - ${l}\n`;
		}
		if (networks.length > 0) {
			yaml += '    networks:\n';
			for (const n of networks) yaml += `      - ${n}\n`;
		}
		if (capAdd.length > 0) {
			yaml += '    cap_add:\n';
			for (const c of capAdd) yaml += `      - ${c}\n`;
		}
		if (capDrop.length > 0) {
			yaml += '    cap_drop:\n';
			for (const c of capDrop) yaml += `      - ${c}\n`;
		}
		if (devices.length > 0) {
			yaml += '    devices:\n';
			for (const d of devices) yaml += `      - ${d}\n`;
		}
		if (tmpfs.length > 0) {
			yaml += '    tmpfs:\n';
			for (const t of tmpfs) yaml += `      - ${t}\n`;
		}
		if (dns.length > 0) {
			yaml += '    dns:\n';
			for (const d of dns) yaml += `      - ${d}\n`;
		}
		if (extraHosts.length > 0) {
			yaml += '    extra_hosts:\n';
			for (const h of extraHosts) yaml += `      - "${h}"\n`;
		}
		if (securityOpt.length > 0) {
			yaml += '    security_opt:\n';
			for (const s of securityOpt) yaml += `      - ${s}\n`;
		}
		if (sysctls.length > 0) {
			yaml += '    sysctls:\n';
			for (const s of sysctls) yaml += `      - ${s}\n`;
		}
		if (logDriver || logOpts.length > 0) {
			yaml += '    logging:\n';
			if (logDriver) yaml += `      driver: ${logDriver}\n`;
			if (logOpts.length > 0) {
				yaml += '      options:\n';
				for (const o of logOpts) {
					const [k, ...v] = o.split('=');
					yaml += `        ${k}: "${v.join('=')}"\n`;
				}
			}
		}
		if (healthCmd) {
			yaml += '    healthcheck:\n';
			yaml += `      test: ["CMD-SHELL", "${healthCmd}"]\n`;
			if (healthInterval) yaml += `      interval: ${healthInterval}\n`;
			if (healthTimeout) yaml += `      timeout: ${healthTimeout}\n`;
			if (healthRetries) yaml += `      retries: ${healthRetries}\n`;
		}
		if (cmdArgs.length > 0) {
			yaml += `    command: ${cmdArgs.join(' ')}\n`;
		}

		// Add named volumes section if any volume uses a named volume (no / or . prefix)
		const namedVolumes = volumes.filter(v => {
			const src = v.split(':')[0];
			return !src.startsWith('/') && !src.startsWith('.') && !src.startsWith('~');
		});
		if (namedVolumes.length > 0) {
			yaml += '\nvolumes:\n';
			for (const v of namedVolumes) {
				const volName = v.split(':')[0];
				yaml += `  ${volName}:\n`;
			}
		}

		// Add networks section if any non-default network
		const customNetworks = networks.filter(n => !['bridge', 'host', 'none'].includes(n));
		if (customNetworks.length > 0) {
			yaml += '\nnetworks:\n';
			for (const n of customNetworks) {
				yaml += `  ${n}:\n    external: true\n`;
			}
		}

		return { name: svcName, yaml };
	}

	function convertDockerRun() {
		convertError = '';
		convertedYaml = '';
		const result = parseDockerRun(dockerRunInput);
		if ('error' in result) {
			convertError = result.error;
		} else {
			convertedYaml = result.yaml;
			convertedName = result.name;
		}
	}

	async function createFromConverted() {
		if (!convertedYaml || !convertedName) return;
		saving = true;
		const r = await api.post<string>(`/env/${$selectedEnv}/stacks`, {
			name: convertedName.toLowerCase().replace(/[^a-z0-9-]/g, '-'),
			compose_content: convertedYaml,
			env_content: null,
		});
		saving = false;
		if (r.success) {
			showRunConvert = false;
			dockerRunInput = '';
			convertedYaml = '';
			toasts.success($t('stacks.created', { name: convertedName }));
			load();
		} else {
			toasts.error(r.error || $t('common.error'));
		}
	}

	function handlePageChange(p: number, pp: number) { page = p; perPage = pp; }
</script>

<svelte:head><title>DockPit — {$t('stacks.title')}</title></svelte:head>

<!-- Summary -->
{#if !loading}
	{@const totalServices = stacks.reduce((a, s) => a + s.services_count, 0)}
	{@const runningServices = stacks.reduce((a, s) => a + s.running_services, 0)}
	<div class="grid grid-cols-2 md:grid-cols-5 gap-3 mb-4">
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-primary">{stacks.length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.title')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-green">{stacks.filter(s => s.status === 'running').length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.active')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-yellow">{stacks.filter(s => s.status === 'partial').length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.partial')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-red">{stacks.filter(s => s.status === 'stopped').length}</div>
			<div class="text-[11px] text-secondary">{$t('stacks.stopped')}</div>
		</div>
		<div class="bg-card border border-theme rounded-lg p-3 flex items-center gap-3">
			<div class="text-xl font-bold text-accent">{runningServices}<span class="text-sm font-normal text-muted">/{totalServices}</span></div>
			<div class="text-[11px] text-secondary">{$t('stacks.services')}</div>
		</div>
	</div>
{/if}

<!-- Table -->
<div class="bg-card border border-theme rounded-lg overflow-hidden">
	<div class="px-4 py-3 border-b border-theme flex items-center justify-between flex-wrap gap-3">
		<h3 class="text-sm font-semibold text-primary">{$t('stacks.title')}</h3>
		<div class="flex items-center gap-2">
			<input bind:value={search} placeholder={$t('common.search')}
				class="bg-[var(--input-bg)] border border-[var(--input-border)] rounded-[var(--radius-md)] px-2.5 py-1.5 text-xs w-44 focus:border-[var(--input-focus)] focus:outline-none focus:shadow-[0_0_0_3px_var(--input-focus-ring)] transition-all duration-200" />
			{#if $canDoAction('action.stack_create_delete')}
				<Button variant="warning" size="sm" onclick={() => showRunConvert = true} title={$t('stacks.fromDockerRun')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
				</Button>
				<Button variant="purple" size="sm" onclick={() => showTemplates = true} title={$t('templates.fromTemplate')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M12 8v8m-4-4h8"/></svg>
				</Button>
				<Button variant="primary" size="sm" onclick={() => { resetCreate(); showCreate = true; }} title={$t('stacks.newStack')}>
					<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				</Button>
			{/if}
			<Button variant="success" size="sm" onclick={load} title={$t('common.refresh')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
			</Button>
		</div>
	</div>

	{#if loading}
		<div class="flex justify-center py-12"><div class="w-5 h-5 border-2 border-theme border-t-[var(--accent)] rounded-full animate-spin"></div></div>
	{:else}
		<div class="overflow-x-auto">
			<table class="w-full">
				<thead><tr class="border-b border-theme">
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('name')}>{$t('common.name')}{sortIndicator('name')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('status')}>{$t('common.status')}{sortIndicator('status')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('services_count')}>{$t('stacks.services')}{sortIndicator('services_count')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold hidden md:table-cell cursor-pointer hover:text-[var(--text)]" onclick={() => toggleSort('path')}>{$t('stacks.path')}{sortIndicator('path')}</th>
					<th class="text-left px-4 py-2.5 text-[10px] uppercase tracking-wider text-muted font-semibold">{$t('common.actions')}</th>
				</tr></thead>
				<tbody>
					{#each paged as s}
						<tr class="border-b border-theme last:border-0 hover:bg-hover transition">
							<td class="px-4 py-3">
								<a href="/stacks/{s.name}" class="text-sm font-medium text-accent hover:text-accent-hover transition">{s.name}</a>
							</td>
							<td class="px-4 py-3"><Badge status={s.status} /></td>
							<td class="px-4 py-3">
								<span class="text-xs">
									<span class="text-green font-medium">{s.running_services}</span>
									<span class="text-muted"> / {s.services_count}</span>
								</span>
							</td>
							<td class="px-4 py-3 text-[11px] font-mono text-muted max-w-[200px] truncate hidden md:table-cell">{s.path}</td>
							<td class="px-4 py-3">
								<div class="flex gap-1">
									{#if $canDoAction('action.stack_deploy_stop')}
										{#if s.status !== 'running'}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--green)] hover:border-[var(--green)]/40 hover:bg-[var(--green)]/8 transition" onclick={() => deploy(s.name)} title={$t('stacks.deploy')}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg></button>
										{:else}
											<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => stop(s.name)} title={$t('containers.stop')}>
												<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1"/></svg></button>
										{/if}
									{/if}
									{#if $canDoAction('action.stack_edit')}
										<a href="/stacks/{s.name}" class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--purple)] hover:border-[var(--purple)]/40 hover:bg-[var(--purple)]/8 transition no-underline" title={$t('common.edit')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg></a>
									{/if}
									{#if $canDoAction('action.stack_migrate')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--accent)] hover:border-[var(--accent)]/40 hover:bg-[var(--accent)]/8 transition" onclick={() => migrateStack = s.name} title={$t('stacks.migrate')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M12 5l7 7-7 7"/></svg></button>
									{/if}
									{#if $canDoAction('action.stack_create_delete')}
										<button class="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 transition" onclick={() => remove(s.name)} title={$t('common.delete')}>
											<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg></button>
									{/if}
								</div>
							</td>
						</tr>
					{:else}
						<tr><td colspan="5" class="text-center py-10 text-sm text-muted">
							{search ? $t('stacks.noStacks') : $t('stacks.noStacksEmpty')}
						</td></tr>
					{/each}
				</tbody>
			</table>
		</div>
		<Pagination total={filtered.length} {page} {perPage} onchange={handlePageChange} />
	{/if}
</div>

<!-- Create Stack Modal -->
{#if showCreate}
	<Modal title={$t('stacks.newStack')} onclose={() => showCreate = false}>
		<div class="mb-4">
			<TextInput bind:value={newName} label={$t('stacks.stackName')} placeholder="e.g. argus, monitoring" id="sn" />
			<p class="text-[10px] text-muted mt-1">{$t('stacks.stackNameHint')}</p>
		</div>

		<div class="flex items-center border border-theme rounded-t-lg overflow-x-auto bg-1">
			{#each tabs as tab, i}
				<button class="flex items-center gap-1 px-3 py-2 text-[11px] font-medium whitespace-nowrap border-b-2 transition
					{createTab === i ? 'border-[var(--accent)] text-accent' : 'border-transparent text-secondary hover:text-primary'}"
					onclick={() => createTab = i}>
					{tab.name}
					{#if tab.removable}
						<span class="ml-1 w-3.5 h-3.5 flex items-center justify-center rounded hover:bg-red-light hover:text-red text-muted cursor-pointer"
							onclick={(e: MouseEvent) => { e.stopPropagation(); removeTab(i); }} role="button" tabindex="-1">
							<svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
						</span>
					{/if}
				</button>
			{/each}
			<button onclick={addFile} class="px-2 py-2 text-[11px] text-muted hover:text-accent transition" title={$t('stacks.addFile')}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
			</button>
		</div>

		{#if createTab === 0 && yamlError}
			<div class="px-3 py-1.5 bg-red-light text-red text-[11px] border-x border-theme">YAML: {yamlError}</div>
		{/if}

		<textarea bind:value={tabs[createTab].content} oninput={onInput} spellcheck={false}
			class="w-full h-[300px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border border-t-0 border-theme rounded-b-lg"
			placeholder={createTab === 0 ? 'services:\n  app:\n    image: ...' : createTab === 1 ? 'KEY=value' : ''}></textarea>

		<div class="flex items-center justify-between mt-1 text-[10px] text-muted px-1">
			<span>{tabs[createTab].name}</span>
			<span>{tabs[createTab].content.split('\n').length} {$t('stacks.lines')}</span>
		</div>

		<div class="flex items-center justify-between mt-4">
			<Button variant="purple" size="sm" onclick={() => { showSaveTemplate = true; }}>
				<svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
				{$t('templates.saveAsTemplate')}
			</Button>
			<div class="flex gap-2">
				<Button variant="danger" size="sm" onclick={() => showCreate = false}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" onclick={create} loading={saving}>{$t('stacks.createStack')}</Button>
			</div>
		</div>
	</Modal>
{/if}

<!-- Template Selector Modal -->
{#if showTemplates}
	<Modal title={$t('templates.selectTemplate')} onclose={() => showTemplates = false} wide>
		{#if templates.length === 0}
			<div class="text-center py-10 text-sm text-muted">{$t('templates.noTemplates')}</div>
		{:else}
			<div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 max-h-[60vh] overflow-y-auto pr-1">
				{#each templates as tpl}
					<button
						class="relative group bg-[var(--bg-1)] border border-theme rounded-xl p-4 text-center transition-all duration-200 hover:border-[var(--accent)] hover:shadow-[0_0_16px_-4px_var(--accent)] cursor-pointer flex flex-col items-center gap-2"
						onclick={() => useTemplate(tpl)}
					>
						{#if !tpl.is_default}
							<button
								class="absolute top-2 right-2 w-5 h-5 flex items-center justify-center rounded-full bg-[var(--bg-0)] border border-theme text-[var(--red)] hover:border-[var(--red)]/40 hover:bg-[var(--red)]/8 opacity-0 group-hover:opacity-100 transition-all z-10"
								onclick={(e) => { e.stopPropagation(); confirmDlg = { message: $t('templates.confirmDelete'), action: () => { confirmDlg = null; deleteTemplate(tpl.id); } }; }}
								title={$t('common.delete')}
							>
								<svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
							</button>
						{/if}
						<div class="w-10 h-10 rounded-lg bg-[var(--bg-0)] border border-theme flex items-center justify-center text-xl shrink-0">
							{tpl.icon || '📦'}
						</div>
						<div class="font-semibold text-xs text-primary truncate w-full">{tpl.name}</div>
						{#if tpl.description}
							<div class="text-[10px] text-muted line-clamp-2 leading-tight">{tpl.description}</div>
						{/if}
						<div class="flex flex-wrap items-center justify-center gap-1 mt-auto pt-1">
							<span class="inline-flex items-center px-1.5 py-0.5 rounded text-[8px] font-medium uppercase tracking-wide bg-[var(--bg-0)] border border-theme text-muted">{tpl.category}</span>
							{#if tpl.is_default}
								<span class="inline-flex items-center px-1.5 py-0.5 rounded text-[8px] font-medium uppercase tracking-wide bg-[color-mix(in_srgb,var(--accent),transparent_85%)] text-accent border border-[color-mix(in_srgb,var(--accent),transparent_70%)]">{$t('templates.default')}</span>
							{:else}
								<span class="inline-flex items-center px-1.5 py-0.5 rounded text-[8px] font-medium uppercase tracking-wide bg-[color-mix(in_srgb,var(--green),transparent_85%)] text-[var(--green)] border border-[color-mix(in_srgb,var(--green),transparent_70%)]">{$t('templates.custom')}</span>
							{/if}
						</div>
					</button>
				{/each}
			</div>
		{/if}
		<div class="flex justify-between items-center mt-4 pt-3 border-t border-theme">
			<Button variant="success" size="sm" onclick={() => { showCreateTemplate = true; }}>
				<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				{$t('templates.createCustom')}
			</Button>
			<Button variant="danger" size="sm" onclick={() => showTemplates = false}>{$t('common.close')}</Button>
		</div>
	</Modal>
{/if}

<!-- Create Custom Template Modal -->
{#if showCreateTemplate}
	<Modal title={$t('templates.createCustom')} onclose={() => showCreateTemplate = false}>
		<div class="space-y-3">
			<div class="flex items-start gap-3">
				<div class="shrink-0">
					<label class="block text-xs font-medium text-secondary mb-1">Icon</label>
					<div class="relative">
						<button
							class="w-10 h-10 rounded-lg bg-[var(--bg-0)] border border-theme flex items-center justify-center text-xl hover:border-[var(--accent)] transition-colors"
							onclick={(e) => { e.preventDefault(); const el = (e.currentTarget as HTMLElement).nextElementSibling; if (el) el.classList.toggle('hidden'); }}
						>{newTemplateIcon}</button>
						<div class="hidden absolute top-12 left-0 z-50 bg-card border border-theme rounded-lg shadow-lg p-2 grid grid-cols-5 gap-1 w-[180px]">
							{#each iconOptions as ico}
								<button
									class="w-8 h-8 rounded-md flex items-center justify-center text-lg hover:bg-[var(--bg-hover)] transition-colors {newTemplateIcon === ico ? 'bg-[var(--bg-hover)] ring-1 ring-[var(--accent)]' : ''}"
									onclick={(e) => { e.preventDefault(); newTemplateIcon = ico; (e.currentTarget as HTMLElement).parentElement?.classList.add('hidden'); }}
								>{ico}</button>
							{/each}
						</div>
					</div>
				</div>
				<div class="flex-1 space-y-3">
					<TextInput bind:value={newTemplateName} label={$t('templates.templateName')} placeholder="e.g. My Web Stack" id="tplname" />
					<TextInput bind:value={newTemplateDesc} label={$t('templates.templateDesc')} placeholder="A short description..." id="tpldesc" />
				</div>
			</div>
			<div>
				<label for="tplcompose" class="block text-xs font-medium text-secondary mb-1">docker-compose.yml</label>
				<textarea id="tplcompose" bind:value={newTemplateCompose} spellcheck={false}
					class="w-full h-[200px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border border-theme rounded-lg"
					placeholder="services:&#10;  app:&#10;    image: nginx:latest"></textarea>
			</div>
			<div>
				<label for="tplenv" class="block text-xs font-medium text-secondary mb-1">.env</label>
				<textarea id="tplenv" bind:value={newTemplateEnv} spellcheck={false}
					class="w-full h-[80px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border border-theme rounded-lg"
					placeholder="KEY=value"></textarea>
			</div>
		</div>
		<div class="flex justify-end gap-2 mt-4">
			<Button variant="danger" size="sm" onclick={() => showCreateTemplate = false}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={saveCustomTemplate}>{$t('common.create')}</Button>
		</div>
	</Modal>
{/if}

<!-- Save as Template Modal -->
{#if showSaveTemplate}
	<Modal title={$t('templates.saveAsTemplate')} onclose={() => showSaveTemplate = false}>
		<div class="space-y-3">
			<TextInput bind:value={saveTemplateName} label={$t('templates.templateName')} placeholder="e.g. My Web Stack" id="savetplname" />
			<TextInput bind:value={saveTemplateDesc} label={$t('templates.templateDesc')} placeholder="A short description..." id="savetpldesc" />
		</div>
		<div class="flex justify-end gap-2 mt-4">
			<Button variant="danger" size="sm" onclick={() => showSaveTemplate = false}>{$t('common.cancel')}</Button>
			<Button variant="primary" size="sm" onclick={saveAsTemplate}>{$t('common.save')}</Button>
		</div>
	</Modal>
{/if}

{#if migrateStack}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-end sm:items-center justify-center z-[1000] p-0 sm:p-4" onclick={(e) => { if (e.target === e.currentTarget) { migrateStack = null; migrateDropdown = false; } }}>
		<div class="border border-[var(--border)] rounded-t-[var(--radius-xl)] sm:rounded-[var(--radius-xl)] w-full sm:max-w-lg shadow-[var(--shadow-lg)]" style="overflow:visible; background:var(--glass-bg); backdrop-filter:blur(20px) saturate(150%); -webkit-backdrop-filter:blur(20px) saturate(150%)">
			<div class="flex items-center justify-between px-5 py-4 border-b border-[var(--border)]">
				<h3 class="text-[15px] font-semibold text-[var(--text)]">{$t('stacks.migrateTitle')}</h3>
				<button class="w-8 h-8 flex items-center justify-center rounded-[var(--radius-md)] border border-[var(--border)] text-[var(--text-secondary)] hover:text-[var(--text)] hover:border-[var(--border-light)] transition-all" onclick={() => { migrateStack = null; migrateDropdown = false; }}>
					<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>
			<div class="p-5 space-y-4">
				<div>
					<p class="text-sm text-secondary mb-1">Stack:</p>
					<p class="text-sm font-medium text-primary">{migrateStack}</p>
				</div>
				<div>
					<label class="block text-xs font-medium text-secondary mb-1">{$t('stacks.migrateTarget')}</label>
					<div class="relative">
						<button
							class="w-full flex items-center gap-2.5 h-9 px-3 text-xs rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--bg-3)] text-[var(--text)] hover:border-[var(--border-light)] hover:shadow-[var(--shadow-sm)] transition-all cursor-pointer"
							onclick={(e) => { e.stopPropagation(); migrateDropdown = !migrateDropdown; }}
						>
							{#if migrateTarget}
								{@const env = $environments.find(e => e.id === migrateTarget)}
								<span class="w-2 h-2 rounded-full shrink-0 {env?.status === 'online' || env?.is_local ? 'bg-[var(--green)] shadow-[var(--shadow-glow-green)]' : 'bg-[var(--red)]'}"></span>
								<span class="truncate font-medium">{env?.name}</span>
							{:else}
								<span class="text-muted">{$t('containers.selectTarget')}</span>
							{/if}
							<svg class="w-3.5 h-3.5 text-muted shrink-0 ml-auto transition-transform duration-200 {migrateDropdown ? 'rotate-180' : ''}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
						</button>
						{#if migrateDropdown}
							<div class="absolute left-0 right-0 mt-1.5 bg-[var(--dropdown-bg)] border border-[var(--border-light)] rounded-[var(--radius-lg)] shadow-[var(--shadow-lg)] z-[1100] py-1.5">
								<div class="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.15em] text-[var(--text-muted)]">{$t('stacks.migrateTarget')}</div>
								{#each migrateTargets as t}
									{@const env = $environments.find(e => e.id === t.value)}
									<button
										class="w-full flex items-center gap-3 px-3 py-2.5 text-xs text-left transition-all duration-150
										{t.value === migrateTarget ? 'bg-[var(--accent-bg)] text-[var(--accent)]' : 'text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text)]'}"
										onclick={() => { migrateTarget = t.value; migrateDropdown = false; }}
									>
										<span class="w-2 h-2 rounded-full shrink-0 {env?.status === 'online' || env?.is_local ? 'bg-[var(--green)]' : 'bg-[var(--red)]'}"></span>
										<span class="truncate font-medium">{t.label}</span>
										{#if env?.is_local}
											<span class="text-[9px] text-muted ml-auto px-1.5 py-0.5 rounded-full bg-[var(--bg-3)]">Local</span>
										{/if}
										{#if t.value === migrateTarget}
											<svg class="w-3.5 h-3.5 shrink-0 ml-auto text-[var(--accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="20 6 9 17 4 12"/></svg>
										{/if}
									</button>
								{/each}
							</div>
						{/if}
					</div>
				</div>
				<div class="space-y-2.5">
					<CustomCheckbox checked={migrateDeploy} onchange={(v) => migrateDeploy = v} label={$t('stacks.migrateDeploy')} />
					<CustomCheckbox checked={migrateStopSource} onchange={(v) => migrateStopSource = v} label={$t('stacks.migrateStopSource')} />
				</div>
			</div>
			<div class="px-5 py-3 border-t border-[var(--border)] flex justify-end gap-2">
				<Button variant="secondary" size="sm" onclick={() => { migrateStack = null; migrateDropdown = false; }}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" onclick={doMigrateStack} loading={migrating} disabled={!migrateTarget}>{$t('stacks.migrateStart')}</Button>
			</div>
		</div>
	</div>
{/if}

{#if showRunConvert}
	<Modal title={$t('stacks.fromDockerRun')} onclose={() => { showRunConvert = false; convertedYaml = ''; convertError = ''; dockerRunInput = ''; }}>
		<div class="space-y-4">
			<div>
				<label class="block text-xs font-medium text-secondary mb-1">Docker Run Command</label>
				<textarea bind:value={dockerRunInput} spellcheck={false}
					class="w-full h-[120px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border border-theme rounded-lg"
					placeholder='docker run -d --name myapp -p 8080:80 -v data:/app/data -e NODE_ENV=production --restart unless-stopped myimage:latest'></textarea>
			</div>
			<Button variant="primary" size="sm" onclick={convertDockerRun}>{$t('stacks.convert')}</Button>
			{#if convertError}
				<p class="text-[var(--red)] text-xs">{convertError}</p>
			{/if}
			{#if convertedYaml}
				<div>
					<div class="flex items-center justify-between mb-1">
						<label class="block text-xs font-medium text-secondary">docker-compose.yml</label>
						<TextInput bind:value={convertedName} label="" placeholder="stack-name" id="cn" class="w-[180px]" />
					</div>
					<textarea bind:value={convertedYaml} spellcheck={false}
						class="w-full h-[300px] bg-0 text-primary font-mono text-[12px] leading-relaxed p-3 resize-none focus:outline-none border border-theme rounded-lg"></textarea>
				</div>
			{/if}
		</div>
		{#if convertedYaml}
			<div class="flex justify-end gap-2 mt-4">
				<Button variant="danger" size="sm" onclick={() => { showRunConvert = false; convertedYaml = ''; convertError = ''; dockerRunInput = ''; }}>{$t('common.cancel')}</Button>
				<Button variant="primary" size="sm" onclick={createFromConverted} loading={saving}>{$t('stacks.createStack')}</Button>
			</div>
		{/if}
	</Modal>
{/if}

{#if confirmDlg}
	<ConfirmDialog message={confirmDlg.message} onconfirm={confirmDlg.action} oncancel={() => confirmDlg = null} />
{/if}
