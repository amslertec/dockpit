use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// === Auth Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

// === Status Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct AppStatus {
    pub setup_complete: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub containers_running: usize,
    pub containers_stopped: usize,
    pub containers_total: usize,
    pub images_total: usize,
    pub volumes_total: usize,
    pub networks_total: usize,
    pub environments: Vec<EnvironmentInfo>,
}

// === Container Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: Vec<PortMapping>,
    pub created: i64,
    pub environment_id: Option<String>,
    pub ip_address: Option<String>,
    pub stack_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortMapping {
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub port_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerAction {
    pub action: String, // start, stop, restart, remove
}

// === Image Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub tags: Vec<String>,
    pub size: f64,
    pub created: i64,
    pub in_use: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullImageRequest {
    pub image: String,
}

// === Volume Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: Option<String>,
    pub in_use: bool,
}

// === Network Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub in_use: bool,
    pub containers_count: usize,
}

// === Docker Registry Login ===

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryLogin {
    pub registry: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryInfo {
    pub registry: String,
    pub username: String,
}

// === Dashboard Widget Data ===

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskUsageInfo {
    pub images_size: f64,
    pub containers_size: f64,
    pub volumes_size: f64,
    pub build_cache_size: f64,
    pub total_size: f64,
}

// === Image Update Check ===

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageUpdateCheck {
    pub outdated: bool,
    pub current_id: String,
    pub latest_id: String,
    pub image: String,
}

// === Stack Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct StackInfo {
    pub name: String,
    pub path: String,
    pub status: String,
    pub services_count: usize,
    pub running_services: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StackDetail {
    pub name: String,
    pub path: String,
    pub status: String,
    pub services_count: usize,
    pub running_services: usize,
    pub compose_content: String,
    pub env_content: Option<String>,
    pub extra_files: Vec<StackFile>,
    pub containers: Vec<ContainerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFile {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStackRequest {
    pub name: String,
    pub compose_content: String,
    pub env_content: Option<String>,
    pub extra_files: Option<Vec<StackFile>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStackRequest {
    pub compose_content: String,
    pub env_content: Option<String>,
    pub extra_files: Option<Vec<StackFile>>,
}

// === Environment / Agent Models ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub status: String, // online, offline
    pub is_local: bool,
    pub agent_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEnvironmentRequest {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub role: String,
    pub totp_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_code: String, // base64 PNG
    pub otpauth_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest2FA {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvStats {
    pub containers_running: usize,
    pub containers_stopped: usize,
    pub containers_total: usize,
    pub images_total: usize,
    pub volumes_total: usize,
    pub networks_total: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub docker_version: String,
    pub os: String,
    pub cpus: i64,
    pub memory_bytes: i64,
    pub memory_display: String,
    pub containers_running: i64,
    pub containers_stopped: i64,
    pub containers_paused: i64,
    pub containers_total: i64,
    pub images: i64,
    pub volumes: usize,
    pub networks: usize,
    pub status: String,
    pub server_type: String,
}

// === User Management Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub role: String,
    pub totp_enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
    pub password: Option<String>,
}

// === Update Monitor Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub id: i64,
    pub container_name: String,
    pub image: String,
    pub server_name: String,
    pub env_id: String,
    pub outdated: bool,
    pub current_id: Option<String>,
    pub latest_id: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckStatus {
    pub running: bool,
    pub total_checked: usize,
    pub total_outdated: usize,
    pub last_check: Option<String>,
}

// === Settings Models ===

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsMap {
    pub settings: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookTestRequest {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}
