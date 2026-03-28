export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
}

export interface LoginResponse {
	token: string;
	username: string;
}

export interface AppStatus {
	setup_complete: boolean;
}

export interface ContainerInfo {
	id: string;
	name: string;
	image: string;
	state: string;
	status: string;
	ports: PortMapping[];
	created: number;
	environment_id?: string;
	ip_address?: string;
	stack_name?: string;
}

export interface PortMapping {
	private_port: number;
	public_port?: number;
	port_type: string;
}

export interface ImageInfo {
	id: string;
	tags: string[];
	size: number;
	created: number;
	in_use: boolean;
}

export interface VolumeInfo {
	name: string;
	driver: string;
	mountpoint: string;
	created?: string;
	in_use: boolean;
}

export interface NetworkInfo {
	id: string;
	name: string;
	driver: string;
	scope: string;
	in_use: boolean;
	containers_count: number;
}

export interface EnvironmentInfo {
	id: string;
	name: string;
	url: string;
	status: string;
	is_local: boolean;
	agent_token?: string;
}

export interface SystemInfo {
	hostname: string;
	docker_version: string;
	os: string;
	cpus: number;
	memory_bytes: number;
	memory_display: string;
	containers_running: number;
	containers_stopped: number;
	containers_paused: number;
	containers_total: number;
	images: number;
	volumes: number;
	networks: number;
	status: string;
	server_type: string;
}

export interface ServerOverview {
	id: string;
	name: string;
	url: string;
	is_local: boolean;
	info: SystemInfo;
}

export interface EnvStats {
	containers_running: number;
	containers_stopped: number;
	containers_total: number;
	images_total: number;
	volumes_total: number;
	networks_total: number;
}

export interface UserProfile {
	username: string;
	role: string;
	totp_enabled: boolean;
}

export interface TotpSetupResponse {
	secret: string;
	qr_code: string;
	otpauth_url: string;
}

export interface RegistryInfo {
	registry: string;
	username: string;
}

export interface DiskUsageInfo {
	images_size: number;
	containers_size: number;
	volumes_size: number;
	build_cache_size: number;
	total_size: number;
}

export interface ImageUpdateCheck {
	outdated: boolean;
	current_id: string;
	latest_id: string;
	image: string;
}

export interface StackInfo {
	name: string;
	path: string;
	status: string;
	services_count: number;
	running_services: number;
}

export interface StackDetail {
	name: string;
	path: string;
	status: string;
	services_count: number;
	running_services: number;
	compose_content: string;
	env_content?: string;
	extra_files: StackFile[];
	containers: ContainerInfo[];
}

export interface StackFile {
	name: string;
	content: string;
}

export interface CreateStackRequest {
	name: string;
	compose_content: string;
	env_content?: string;
	extra_files?: StackFile[];
}

export interface UpdateStackRequest {
	compose_content: string;
	env_content?: string;
	extra_files?: StackFile[];
}

// === User Management ===

export interface UserInfo {
	id: string;
	username: string;
	role: string;
	totp_enabled: boolean;
	created_at: string;
}

export interface CreateUserRequest {
	username: string;
	password: string;
	role: string;
}

export interface UpdateUserRequest {
	role?: string;
	password?: string;
}

// === Update Monitor ===

export interface UpdateCheckResult {
	id: number;
	container_name: string;
	image: string;
	server_name: string;
	env_id: string;
	outdated: boolean;
	current_id?: string;
	latest_id?: string;
	checked_at: string;
}

// === Settings ===

export interface SettingsMap {
	settings: Record<string, string>;
}

// === Live Stats ===

export interface ContainerStats {
	id: string;
	name: string;
	cpu_percent: number;
	memory_usage: number;
	memory_limit: number;
	memory_percent: number;
	network_rx: number;
	network_tx: number;
	block_read: number;
	block_write: number;
}

export interface StatsSnapshot {
	containers: ContainerStats[];
	timestamp: number;
}

export interface NotificationInfo {
	id: number;
	type: string;
	title: string;
	message: string;
	read: boolean;
	created_at: string;
}

export interface ScheduledJob {
	id: string;
	env_id: string;
	job_type: string;
	enabled: boolean;
	interval_hours: number;
	stack_name?: string;
	last_run?: string;
	next_run?: string;
	last_result?: string;
	last_message?: string;
}

// === Container Events ===

export interface ContainerEvent {
	id?: number;
	env_id: string;
	container_id?: string;
	container_name?: string;
	event_type: string;
	event_action: string;
	details?: string;
	timestamp: string;
}

export interface EventsResponse {
	events: ContainerEvent[];
	total: number;
}

// === Vulnerability Scanning ===

export interface VulnerabilityScan {
	id?: number;
	env_id: string;
	image: string;
	critical: number;
	high: number;
	medium: number;
	low: number;
	total: number;
	cves_json?: string;
	scanned_at?: string;
}
