//! DockPit Agent - Runs on remote Docker hosts.
//!
//! Start on a remote server and connect it from the DockPit management UI.
//! No configuration required - just start it and enter the IP:Port in DockPit.
//!
//! Usage:
//!   docker run -d -p 5522:5522 -v /var/run/docker.sock:/var/run/docker.sock dockpit-agent

use base64::Engine;
use axum::{
    Router,
    extract::{Path, Query, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json,
};
use std::sync::{Arc, RwLock};

use bollard::Docker;
use bollard::container::{
    ListContainersOptions, LogsOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// === Models ===

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self { success: false, data: None, error: Some(msg.into()) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ContainerInfo {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    ports: Vec<PortMapping>,
    created: i64,
    environment_id: Option<String>,
    ip_address: Option<String>,
    stack_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PortMapping {
    private_port: u16,
    public_port: Option<u16>,
    port_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContainerAction {
    action: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageInfo {
    id: String,
    tags: Vec<String>,
    size: f64,
    created: i64,
    in_use: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentInfo {
    hostname: String,
    version: String,
    docker_version: String,
    paired: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct PairRequest {
    token: String,
}

// === State ===

struct AgentState {
    docker: Docker,
    hostname: String,
    paired_token: RwLock<Option<String>>,
    stacks_dir: std::path::PathBuf,
}

impl AgentState {
    fn check_auth(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        let lock = self.paired_token.read().unwrap();
        let stored = match lock.as_ref() {
            Some(t) => t.clone(),
            None => return Ok(()), // Not paired yet, allow all
        };
        drop(lock);

        let provided = headers
            .get("X-Agent-Token")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(t) if t == stored => Ok(()),
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

// === Public Handlers ===

async fn health(State(state): State<Arc<AgentState>>) -> Json<ApiResponse<AgentInfo>> {
    let docker_version = match state.docker.version().await {
        Ok(v) => v.version.unwrap_or_else(|| "unknown".to_string()),
        Err(_) => "unavailable".to_string(),
    };

    let paired = state.paired_token.read().unwrap().is_some();

    Json(ApiResponse::ok(AgentInfo {
        hostname: state.hostname.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        docker_version,
        paired,
    }))
}

async fn pair(
    State(state): State<Arc<AgentState>>,
    Json(req): Json<PairRequest>,
) -> Json<ApiResponse<String>> {
    let mut lock = state.paired_token.write().unwrap();

    if lock.is_some() {
        return Json(ApiResponse::err("Agent is already paired. Restart agent to re-pair."));
    }

    if req.token.len() < 16 {
        return Json(ApiResponse::err("Token too short"));
    }

    *lock = Some(req.token);
    tracing::info!("Successfully paired with management server");
    Json(ApiResponse::ok("Paired successfully".to_string()))
}

// === Protected Handlers ===

async fn list_containers(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<ContainerInfo>>>, StatusCode> {
    state.check_auth(&headers)?;

    let mut filters = HashMap::new();
    filters.insert("status", vec!["running", "exited", "paused", "created", "restarting", "dead"]);

    let options = ListContainersOptions { all: true, filters, ..Default::default() };

    match state.docker.list_containers(Some(options)).await {
        Ok(containers) => {
            let infos: Vec<ContainerInfo> = containers
                .into_iter()
                .map(|c| {
                    let name = c.names
                        .as_ref()
                        .and_then(|n| n.first())
                        .map(|n| n.trim_start_matches('/').to_string())
                        .unwrap_or_default();

                    let ports = c.ports
                        .unwrap_or_default()
                        .into_iter()
                        .map(|p| PortMapping {
                            private_port: p.private_port,
                            public_port: p.public_port,
                            port_type: p.typ.map(|t| format!("{:?}", t)).unwrap_or_else(|| "tcp".to_string()),
                        })
                        .collect();

                    let ip_address = c.network_settings
                        .as_ref()
                        .and_then(|ns| ns.networks.as_ref())
                        .and_then(|nets| nets.values().next())
                        .and_then(|net| net.ip_address.clone())
                        .filter(|ip| !ip.is_empty());

                    let stack_name = c.labels
                        .as_ref()
                        .and_then(|l| l.get("com.docker.compose.project"))
                        .cloned();

                    ContainerInfo {
                        id: c.id.unwrap_or_default(),
                        name,
                        image: c.image.unwrap_or_default(),
                        state: c.state.unwrap_or_default(),
                        status: c.status.unwrap_or_default(),
                        ports,
                        created: c.created.unwrap_or(0),
                        environment_id: None,
                        ip_address,
                        stack_name,
                    }
                })
                .collect();
            Ok(Json(ApiResponse::ok(infos)))
        }
        Err(e) => Ok(Json(ApiResponse::err(format!("Failed: {}", e)))),
    }
}

async fn container_action(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(action): Json<ContainerAction>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;

    let result = match action.action.as_str() {
        "start" => state.docker.start_container(&id, None::<StartContainerOptions<String>>).await,
        "stop" => state.docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await,
        "restart" => state.docker.restart_container(&id, Some(bollard::container::RestartContainerOptions { t: 10 })).await,
        "remove" => state.docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await,
        _ => return Ok(Json(ApiResponse::err("Invalid action"))),
    };

    match result {
        Ok(_) => Ok(Json(ApiResponse::ok(format!("{} successful", action.action)))),
        Err(e) => Ok(Json(ApiResponse::err(format!("Failed: {}", e)))),
    }
}

async fn container_logs(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;

    let options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: "200".to_string(),
        timestamps: true,
        ..Default::default()
    };

    let mut stream = state.docker.logs(&id, Some(options));
    let mut logs = String::new();
    while let Some(Ok(log)) = stream.next().await {
        logs.push_str(&log.to_string());
    }

    Ok(Json(ApiResponse::ok(logs)))
}

async fn list_images(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<ImageInfo>>>, StatusCode> {
    state.check_auth(&headers)?;

    use bollard::image::ListImagesOptions;

    let options = ListImagesOptions::<String> { all: false, ..Default::default() };

    // Get used image IDs
    use bollard::container::ListContainersOptions as LCO;
    let containers = state.docker.list_containers(Some(LCO::<&str> { all: true, ..Default::default() })).await.unwrap_or_default();
    let used_ids: std::collections::HashSet<String> = containers.iter().filter_map(|c| c.image_id.clone()).collect();

    match state.docker.list_images(Some(options)).await {
        Ok(images) => {
            let infos: Vec<ImageInfo> = images
                .into_iter()
                .map(|img| {
                    let in_use = used_ids.contains(&img.id);
                    ImageInfo {
                        id: img.id[..std::cmp::min(19, img.id.len())].to_string(),
                        tags: img.repo_tags,
                        size: img.size as f64 / 1_000_000.0,
                        created: img.created,
                        in_use,
                    }
                })
                .collect();
            Ok(Json(ApiResponse::ok(infos)))
        }
        Err(e) => Ok(Json(ApiResponse::err(format!("Failed: {}", e)))),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUpdateCheck {
    outdated: bool,
    current_id: String,
    latest_id: String,
    image: String,
}

#[derive(Debug, Deserialize)]
struct CheckUpdateBody {
    credentials: Option<Vec<RegistryCredential>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryCredential {
    registry: String,
    username: String,
    password: String,
}

async fn check_container_update(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<CheckUpdateBody>>,
) -> Result<Json<ApiResponse<ImageUpdateCheck>>, StatusCode> {
    state.check_auth(&headers)?;

    let container = state.docker.inspect_container(&id, None).await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let image_name = container.config
        .and_then(|c| c.image)
        .unwrap_or_default();

    let local_image = state.docker.inspect_image(&image_name).await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let local_id = local_image.id.unwrap_or_default();
    let local_digests = local_image.repo_digests.unwrap_or_default();

    if local_digests.is_empty() {
        return Ok(Json(ApiResponse::ok(ImageUpdateCheck {
            outdated: false, current_id: local_id.clone(), latest_id: local_id, image: image_name,
        })));
    }

    let (registry, repo, tag) = parse_image_ref(&image_name);

    // Extract credentials sent by DockPit server
    let creds = body.and_then(|Json(b)| b.credentials).unwrap_or_else(Vec::new);

    let remote_config_digest = match fetch_remote_config_digest(&registry, &repo, &tag, &creds).await {
        Ok(d) => d,
        Err(_) => {
            return Ok(Json(ApiResponse::ok(ImageUpdateCheck {
                outdated: false, current_id: local_id.clone(), latest_id: local_id, image: image_name,
            })));
        }
    };

    let local_clean = local_id.trim_start_matches("sha256:").to_string();
    let remote_clean = remote_config_digest.trim_start_matches("sha256:").to_string();
    let outdated = !remote_clean.is_empty() && local_clean != remote_clean;

    Ok(Json(ApiResponse::ok(ImageUpdateCheck {
        outdated, current_id: local_id, latest_id: remote_config_digest, image: image_name,
    })))
}

fn parse_image_ref(image: &str) -> (String, String, String) {
    let (name, tag) = if let Some((n, t)) = image.rsplit_once(':') {
        if t.contains('/') { (image, "latest") } else { (n, t) }
    } else {
        (image, "latest")
    };
    let parts: Vec<&str> = name.splitn(2, '/').collect();
    if parts.len() == 1 {
        ("registry-1.docker.io".into(), format!("library/{}", parts[0]), tag.into())
    } else if parts[0].contains('.') || parts[0].contains(':') {
        (parts[0].to_string(), parts[1].to_string(), tag.into())
    } else {
        ("registry-1.docker.io".into(), name.to_string(), tag.into())
    }
}

/// Find credentials for a registry from the list sent by DockPit server
fn find_credentials(registry: &str, creds: &[RegistryCredential]) -> Option<(String, String)> {
    creds.iter().find(|c| {
        let r = c.registry.to_lowercase();
        if registry.contains("docker.io") || registry.contains("registry-1") {
            r == "docker.io" || r.contains("docker.io") || r.contains("index.docker.io")
        } else {
            r.contains(registry) || registry.contains(&r)
        }
    }).map(|c| (c.username.clone(), c.password.clone()))
}

async fn get_registry_token_with_creds(client: &reqwest::Client, registry: &str, repo: &str, creds: &[RegistryCredential]) -> Result<String, String> {
    let auth = find_credentials(registry, creds);

    if registry.contains("docker.io") || registry.contains("registry-1") {
        let url = format!("https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull", repo);
        let mut req = client.get(&url);
        if let Some((user, pass)) = &auth { req = req.basic_auth(user, Some(pass)); }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let token = data["token"].as_str().unwrap_or("").to_string();
        if token.is_empty() { return Err("No token".into()); }
        return Ok(token);
    }
    if registry.contains("ghcr.io") {
        let url = format!("https://ghcr.io/token?service=ghcr.io&scope=repository:{}:pull", repo);
        let mut req = client.get(&url);
        if let Some((user, pass)) = &auth { req = req.basic_auth(user, Some(pass)); }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let token = data["token"].as_str().unwrap_or("").to_string();
        if token.is_empty() { return Err("No token".into()); }
        return Ok(token);
    }
    if let Some((user, pass)) = auth {
        return Ok(format!("Basic {}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("{}:{}", user, pass))));
    }
    Ok(String::new())
}

async fn fetch_remote_config_digest(registry: &str, repo: &str, tag: &str, creds: &[RegistryCredential]) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    let token = get_registry_token_with_creds(&client, registry, repo, creds).await.unwrap_or_default();
    let auth_header = if !token.is_empty() { format!("Bearer {}", token) } else { String::new() };

    // GET manifest list / OCI index
    let url = format!("https://{}/v2/{}/manifests/{}", registry, repo, tag);
    let mut req = client.get(&url)
        .header("Accept", "application/vnd.oci.image.index.v1+json")
        .header("Accept", "application/vnd.docker.distribution.manifest.list.v2+json")
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
        .header("Accept", "application/vnd.oci.image.manifest.v1+json");
    if !auth_header.is_empty() { req = req.header("Authorization", &auth_header); }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Registry returned {}", resp.status()));
    }

    let content_type = resp.headers().get("content-type")
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    if content_type.contains("manifest.list") || content_type.contains("image.index") {
        let manifests = body["manifests"].as_array().ok_or("No manifests in index")?;
        let amd64 = manifests.iter().find(|m| {
            let p = &m["platform"];
            p["architecture"].as_str().unwrap_or("") == "amd64"
                && p["os"].as_str().unwrap_or("") == "linux"
                && !m["mediaType"].as_str().unwrap_or("").contains("attestation")
        }).ok_or("No linux/amd64 manifest")?;
        let manifest_digest = amd64["digest"].as_str().ok_or("No digest")?;

        let manifest_url = format!("https://{}/v2/{}/manifests/{}", registry, repo, manifest_digest);
        let mut req = client.get(&manifest_url)
            .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
            .header("Accept", "application/vnd.oci.image.manifest.v1+json");
        if !auth_header.is_empty() { req = req.header("Authorization", &auth_header); }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() { return Err(format!("Registry returned {}", resp.status())); }
        let manifest: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        manifest["config"]["digest"].as_str().map(|s| s.to_string()).ok_or_else(|| "No config digest".into())
    } else {
        body["config"]["digest"].as_str().map(|s| s.to_string()).ok_or_else(|| "No config digest".into())
    }
}

async fn recreate_container(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;

    // Check if this is a self-update (agent trying to recreate itself)
    let own_hostname = &state.hostname;
    let is_self = id.starts_with(own_hostname) || own_hostname.starts_with(&id[..std::cmp::min(id.len(), 12)]);

    let inspect = state.docker.inspect_container(&id, None).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let stack = inspect.config.as_ref().and_then(|c| c.labels.as_ref()).and_then(|l| l.get("com.docker.compose.project")).cloned();
    let service = inspect.config.as_ref().and_then(|c| c.labels.as_ref()).and_then(|l| l.get("com.docker.compose.service")).cloned();

    // Self-update: pull image, then use docker CLI to recreate in background
    if is_self {
        let image = inspect.config.as_ref().and_then(|c| c.image.clone()).unwrap_or_default();
        let name = inspect.name.clone().unwrap_or_default().trim_start_matches('/').to_string();

        // Step 1: Pull new image first
        use bollard::image::CreateImageOptions;
        let (repo, tag) = if let Some((r, t)) = image.split_once(':') { (r.to_string(), t.to_string()) } else { (image.clone(), "latest".to_string()) };
        let mut stream = state.docker.create_image(Some(CreateImageOptions { from_image: repo, tag, ..Default::default() }), None, None);
        while let Some(r) = futures_lite::StreamExt::next(&mut stream).await { let _ = r; }

        // Step 2: If compose service, use compose to recreate (handles everything)
        if let (Some(ref stack_name), Some(ref svc)) = (&stack, &service) {
            let dir = state.stacks_dir.join(stack_name);
            if dir.exists() {
                // Spawn background compose up (will replace this container)
                let dir_str = dir.to_string_lossy().to_string();
                let svc_clone = svc.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    let _ = tokio::process::Command::new("docker")
                        .args(["compose", "up", "-d", "--force-recreate", "--pull", "always", &svc_clone])
                        .current_dir(&dir_str)
                        .output().await;
                });
                return Ok(Json(ApiResponse::ok(format!("Self-update initiated for '{}'. Agent will restart in ~5 seconds.", name))));
            }
        }

        // Standalone: spawn a helper container to do the swap
        let docker = state.docker.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            // Stop ourselves
            let _ = docker.stop_container(&id_clone, Some(bollard::container::StopContainerOptions { t: 5 })).await;
            // Note: after this point, this task will be killed as the container stops
        });
        return Ok(Json(ApiResponse::ok(format!(
            "Self-update: Image pulled. Container '{}' will stop now. Please restart it manually with: docker start {}",
            name, name
        ))));
    }

    if let (Some(stack_name), Some(svc)) = (stack, service) {
        let dir = state.stacks_dir.join(&stack_name);
        if dir.exists() {
            let mut lines = vec![format!("Stack: {}, Service: {}", stack_name, svc), String::new()];

            lines.push("→ docker compose pull...".into());
            if let Ok(p) = tokio::process::Command::new("docker").args(["compose", "pull", &svc]).current_dir(&dir).output().await {
                for l in String::from_utf8_lossy(&p.stdout).lines().chain(String::from_utf8_lossy(&p.stderr).lines()) {
                    if !l.trim().is_empty() { lines.push(format!("  {}", l)); }
                }
            }

            lines.push(String::new());
            lines.push("→ docker compose up --force-recreate...".into());
            let out = tokio::process::Command::new("docker").args(["compose", "up", "-d", "--force-recreate", &svc]).current_dir(&dir).output().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            for l in String::from_utf8_lossy(&out.stdout).lines().chain(String::from_utf8_lossy(&out.stderr).lines()) {
                if !l.trim().is_empty() { lines.push(format!("  {}", l)); }
            }

            let full = lines.join("\n");
            return if out.status.success() { Ok(Json(ApiResponse::ok(full))) } else { Ok(Json(ApiResponse::err(full))) };
        }
    }

    // Standalone: pull, stop, remove, create, start
    let config = inspect.config.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let image = config.image.clone().unwrap_or_default();
    let name = inspect.name.unwrap_or_default().trim_start_matches('/').to_string();
    let host_config = inspect.host_config;
    let net_config = inspect.network_settings.and_then(|ns| ns.networks).map(|nets| {
        let eps: std::collections::HashMap<String, bollard::models::EndpointSettings> = nets.into_iter().map(|(k, v)| (k, bollard::models::EndpointSettings { aliases: v.aliases, network_id: v.network_id, ..Default::default() })).collect();
        bollard::container::NetworkingConfig { endpoints_config: eps }
    });

    // Pull
    use bollard::image::CreateImageOptions;
    let (repo, tag) = if let Some((r, t)) = image.split_once(':') { (r.to_string(), t.to_string()) } else { (image.clone(), "latest".to_string()) };
    let mut stream = state.docker.create_image(Some(CreateImageOptions { from_image: repo, tag, ..Default::default() }), None, None);
    while let Some(r) = futures_lite::StreamExt::next(&mut stream).await { let _ = r; }

    // Stop & remove
    let _ = state.docker.stop_container(&id, Some(bollard::container::StopContainerOptions { t: 10 })).await;
    state.docker.remove_container(&id, Some(bollard::container::RemoveContainerOptions { force: true, ..Default::default() })).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create
    let body = bollard::container::Config {
        image: Some(image), hostname: config.hostname, domainname: config.domainname, user: config.user,
        env: config.env, cmd: config.cmd, entrypoint: config.entrypoint, working_dir: config.working_dir,
        labels: config.labels, exposed_ports: config.exposed_ports, volumes: config.volumes,
        tty: config.tty, open_stdin: config.open_stdin, host_config, networking_config: net_config, ..Default::default()
    };
    let created = state.docker.create_container(Some(bollard::container::CreateContainerOptions { name: name.clone(), ..Default::default() }), body).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.docker.start_container(&created.id, None::<bollard::container::StartContainerOptions<String>>).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::ok(format!("Container '{}' recreated", name))))
}

async fn remove_image(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    let force = query.get("force").map(|v| v == "true").unwrap_or(false);
    use bollard::image::RemoveImageOptions;
    match state.docker.remove_image(&id, Some(RemoveImageOptions { force, ..Default::default() }), None).await {
        Ok(_) => Ok(Json(ApiResponse::ok("Image gelöscht".into()))),
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

async fn prune_images(State(state): State<Arc<AgentState>>, headers: HeaderMap) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    let result = state.docker.prune_images::<String>(None).await;
    match result {
        Ok(r) => {
            let deleted = r.images_deleted.map(|v| v.len()).unwrap_or(0);
            let space = r.space_reclaimed.unwrap_or(0);
            Ok(Json(ApiResponse::ok(format!("{} Images gelöscht, {:.1} MB freigegeben", deleted, space as f64 / 1_000_000.0))))
        }
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

// === Volumes & Networks ===

#[derive(Debug, Serialize, Deserialize)]
struct VolumeInfo {
    name: String,
    driver: String,
    mountpoint: String,
    created: Option<String>,
    in_use: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkInfo {
    id: String,
    name: String,
    driver: String,
    scope: String,
    in_use: bool,
    containers_count: usize,
}

async fn list_volumes(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<VolumeInfo>>>, StatusCode> {
    state.check_auth(&headers)?;

    use bollard::volume::ListVolumesOptions;
    use bollard::container::ListContainersOptions as LCO2;

    let containers = state.docker.list_containers(Some(LCO2::<&str> { all: true, ..Default::default() })).await.unwrap_or_default();
    let used_vols: std::collections::HashSet<String> = containers.iter()
        .filter_map(|c| c.mounts.as_ref())
        .flat_map(|m| m.iter())
        .filter_map(|m| m.name.clone())
        .collect();

    match state.docker.list_volumes(Some(ListVolumesOptions::<String> { ..Default::default() })).await {
        Ok(result) => {
            let vols: Vec<VolumeInfo> = result.volumes.unwrap_or_default()
                .into_iter()
                .map(|v| {
                    let in_use = used_vols.contains(&v.name);
                    VolumeInfo { name: v.name, driver: v.driver, mountpoint: v.mountpoint, created: v.created_at, in_use }
                })
                .collect();
            Ok(Json(ApiResponse::ok(vols)))
        }
        Err(e) => Ok(Json(ApiResponse::err(format!("Failed: {}", e)))),
    }
}

async fn remove_volume(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(name): Path<String>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    let force = query.get("force").map(|v| v == "true").unwrap_or(false);
    use bollard::volume::RemoveVolumeOptions;
    match state.docker.remove_volume(&name, Some(RemoveVolumeOptions { force })).await {
        Ok(_) => Ok(Json(ApiResponse::ok("Volume gelöscht".into()))),
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

async fn prune_volumes(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    match state.docker.prune_volumes::<String>(None).await {
        Ok(r) => {
            let deleted = r.volumes_deleted.map(|v| v.len()).unwrap_or(0);
            let space = r.space_reclaimed.unwrap_or(0);
            Ok(Json(ApiResponse::ok(format!("{} Volumes gelöscht, {:.1} MB freigegeben", deleted, space as f64 / 1_000_000.0))))
        }
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

async fn list_networks(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<NetworkInfo>>>, StatusCode> {
    state.check_auth(&headers)?;

    use bollard::network::ListNetworksOptions;
    match state.docker.list_networks(Some(ListNetworksOptions::<String> { ..Default::default() })).await {
        Ok(networks) => {
            // Get used networks from containers
            use bollard::container::ListContainersOptions as LCO3;
            let ctrs = state.docker.list_containers(Some(LCO3::<&str> { all: true, ..Default::default() })).await.unwrap_or_default();
            let mut net_usage: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for c in &ctrs {
                if let Some(ns) = c.network_settings.as_ref().and_then(|s| s.networks.as_ref()) {
                    for (name, _) in ns { *net_usage.entry(name.clone()).or_insert(0) += 1; }
                }
            }

            let nets: Vec<NetworkInfo> = networks
                .into_iter()
                .map(|n| {
                    let name = n.name.unwrap_or_default();
                    let cc = net_usage.get(&name).copied().unwrap_or(0);
                    NetworkInfo {
                        id: n.id.unwrap_or_default(),
                        name,
                        driver: n.driver.unwrap_or_default(),
                        scope: n.scope.unwrap_or_default(),
                        in_use: cc > 0,
                        containers_count: cc,
                    }
                })
                .collect();
            Ok(Json(ApiResponse::ok(nets)))
        }
        Err(e) => Ok(Json(ApiResponse::err(format!("Failed: {}", e)))),
    }
}

async fn remove_network(
    State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    match state.docker.remove_network(&id).await {
        Ok(_) => Ok(Json(ApiResponse::ok("Netzwerk gelöscht".into()))),
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

async fn prune_networks(
    State(state): State<Arc<AgentState>>, headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    match state.docker.prune_networks::<String>(None).await {
        Ok(r) => Ok(Json(ApiResponse::ok(format!("{} Netzwerke gelöscht", r.networks_deleted.map(|v| v.len()).unwrap_or(0))))),
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

// === Stacks ===

#[derive(Debug, Serialize, Deserialize)]
struct StackInfo { name: String, path: String, status: String, services_count: usize, running_services: usize }
#[derive(Debug, Serialize, Deserialize)]
struct StackDetail { name: String, path: String, status: String, services_count: usize, running_services: usize, compose_content: String, env_content: Option<String>, extra_files: Vec<StackFile>, containers: Vec<ContainerInfo> }
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StackFile { name: String, content: String }
#[derive(Debug, Serialize, Deserialize)]
struct CreateStackReq { name: String, compose_content: String, env_content: Option<String>, extra_files: Option<Vec<StackFile>> }
#[derive(Debug, Serialize, Deserialize)]
struct UpdateStackReq { compose_content: String, env_content: Option<String>, extra_files: Option<Vec<StackFile>> }

const COMPOSE_NAMES: &[&str] = &["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"];

fn find_compose(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    COMPOSE_NAMES.iter().map(|n| dir.join(n)).find(|p| p.exists())
}

fn validate_stack_name(n: &str) -> Result<(), String> {
    if n.is_empty() || n.contains("..") || n.contains('/') || n.contains('\\')
        || !n.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    { Err("Ungültiger Stack-Name".into()) } else { Ok(()) }
}

fn validate_filename(n: &str) -> Result<(), String> {
    if n.is_empty() || n.contains("..") || n.contains('/') || n.contains('\\') { Err(format!("Ungültiger Dateiname: {}", n)) } else { Ok(()) }
}

fn count_yaml_services(content: &str) -> usize {
    serde_yaml::from_str::<serde_yaml::Value>(content).ok()
        .and_then(|v| v.get("services")?.as_mapping().map(|m| m.len())).unwrap_or(0)
}

async fn get_stack_containers_by_name(name: &str) -> Vec<ContainerInfo> {
    let project = name.to_lowercase();
    let out = tokio::process::Command::new("docker")
        .args(["ps", "-a", "--filter", &format!("label=com.docker.compose.project={}", project),
               "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Status}}\t{{.Ports}}\t{{.CreatedAt}}\t{{.Label \"com.docker.compose.project\"}}"])
        .output().await;
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).lines()
                .filter(|l| !l.trim().is_empty())
                .map(|line| {
                    let p: Vec<&str> = line.splitn(8, '\t').collect();
                    ContainerInfo {
                        id: p.first().unwrap_or(&"").to_string(),
                        name: p.get(1).unwrap_or(&"").to_string(),
                        image: p.get(2).unwrap_or(&"").to_string(),
                        state: p.get(3).unwrap_or(&"").to_string(),
                        status: p.get(4).unwrap_or(&"").to_string(),
                        ports: vec![],
                        created: 0,
                        environment_id: None,
                        ip_address: None, // Not available from docker ps
                        stack_name: p.get(7).map(|s| s.to_string()).filter(|s| !s.is_empty()),
                    }
                }).collect()
        }
        _ => vec![],
    }
}

async fn get_compose_status(name: &str) -> (String, usize) {
    let containers = get_stack_containers_by_name(name).await;
    if containers.is_empty() { return ("stopped".into(), 0); }
    let running = containers.iter().filter(|c| c.state == "running").count();
    let status = if running == containers.len() { "running" } else if running > 0 { "partial" } else { "stopped" };
    (status.into(), running)
}

async fn agent_list_stacks(State(state): State<Arc<AgentState>>, headers: HeaderMap) -> Result<Json<ApiResponse<Vec<StackInfo>>>, StatusCode> {
    state.check_auth(&headers)?;
    let entries = std::fs::read_dir(&state.stacks_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut stacks = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || find_compose(&path).is_none() { continue; }
        let name = entry.file_name().to_string_lossy().to_string();
        let compose = find_compose(&path).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let sc = count_yaml_services(&compose);
        let (status, running) = get_compose_status(&name).await;
        stacks.push(StackInfo { name, path: path.to_string_lossy().to_string(), status, services_count: sc, running_services: running });
    }
    stacks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(ApiResponse::ok(stacks)))
}

async fn agent_get_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(name): Path<String>) -> Result<Json<ApiResponse<StackDetail>>, StatusCode> {
    state.check_auth(&headers)?;
    validate_stack_name(&name).map_err(|_| StatusCode::BAD_REQUEST)?;
    let dir = state.stacks_dir.join(&name);
    if !dir.exists() { return Ok(Json(ApiResponse::err("Stack nicht gefunden"))); }
    let cf = find_compose(&dir).ok_or(StatusCode::NOT_FOUND)?;
    let compose_content = std::fs::read_to_string(&cf).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sc = count_yaml_services(&compose_content);
    let (status, running) = get_compose_status(&name).await;
    let containers = get_stack_containers_by_name(&name).await;
    let env_content = { let p = dir.join(".env"); if p.exists() { std::fs::read_to_string(p).ok() } else { None } };
    let cn = cf.file_name().unwrap().to_string_lossy().to_string();
    let mut extra = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let f = e.file_name().to_string_lossy().to_string();
            if f == cn || f == ".env" || f.starts_with('.') || e.path().is_dir() { continue; }
            if let Ok(c) = std::fs::read_to_string(e.path()) { extra.push(StackFile { name: f, content: c }); }
        }
    }
    extra.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(ApiResponse::ok(StackDetail { name: name.clone(), path: dir.to_string_lossy().to_string(), status, services_count: sc, running_services: running, compose_content, env_content, extra_files: extra, containers })))
}

async fn agent_create_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Json(req): Json<CreateStackReq>) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    if let Err(e) = validate_stack_name(&req.name) { return Ok(Json(ApiResponse::err(e))); }
    if req.compose_content.contains('\t') { return Ok(Json(ApiResponse::err("Tabs nicht erlaubt in YAML"))); }
    if serde_yaml::from_str::<serde_yaml::Value>(&req.compose_content).is_err() { return Ok(Json(ApiResponse::err("YAML-Syntaxfehler"))); }
    let dir = state.stacks_dir.join(&req.name);
    if dir.exists() { return Ok(Json(ApiResponse::err("Existiert bereits"))); }
    std::fs::create_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(dir.join("docker-compose.yml"), &req.compose_content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(ref env) = req.env_content { if !env.trim().is_empty() { std::fs::write(dir.join(".env"), env).ok(); } }
    if let Some(ref files) = req.extra_files { for f in files { if validate_filename(&f.name).is_ok() { std::fs::write(dir.join(&f.name), &f.content).ok(); } } }
    Ok(Json(ApiResponse::ok("Erstellt".into())))
}

async fn agent_update_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(name): Path<String>, Json(req): Json<UpdateStackReq>) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    if let Err(e) = validate_stack_name(&name) { return Ok(Json(ApiResponse::err(e))); }
    if serde_yaml::from_str::<serde_yaml::Value>(&req.compose_content).is_err() { return Ok(Json(ApiResponse::err("YAML-Syntaxfehler"))); }
    let dir = state.stacks_dir.join(&name);
    if !dir.exists() { return Ok(Json(ApiResponse::err("Nicht gefunden"))); }
    let cp = find_compose(&dir).unwrap_or_else(|| dir.join("docker-compose.yml"));
    std::fs::write(&cp, &req.compose_content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ep = dir.join(".env");
    match &req.env_content { Some(c) if !c.trim().is_empty() => { std::fs::write(&ep, c).ok(); }, _ => { if ep.exists() { std::fs::remove_file(&ep).ok(); } } }
    // Handle extra files
    let cn = cp.file_name().unwrap().to_string_lossy().to_string();
    let new_names: Vec<String> = req.extra_files.as_ref().map(|f| f.iter().map(|x| x.name.clone()).collect()).unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(&dir) { for e in entries.flatten() { let f = e.file_name().to_string_lossy().to_string(); if f == cn || f == ".env" || f.starts_with('.') || e.path().is_dir() { continue; } if !new_names.contains(&f) { std::fs::remove_file(e.path()).ok(); } } }
    if let Some(ref files) = req.extra_files { for f in files { if validate_filename(&f.name).is_ok() { std::fs::write(dir.join(&f.name), &f.content).ok(); } } }
    Ok(Json(ApiResponse::ok("Aktualisiert".into())))
}

async fn agent_delete_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(name): Path<String>) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    if let Err(e) = validate_stack_name(&name) { return Ok(Json(ApiResponse::err(e))); }
    let dir = state.stacks_dir.join(&name);
    if !dir.exists() { return Ok(Json(ApiResponse::err("Nicht gefunden"))); }
    std::fs::remove_dir_all(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(ApiResponse::ok("Gelöscht".into())))
}

async fn agent_deploy_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(name): Path<String>) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    if let Err(e) = validate_stack_name(&name) { return Ok(Json(ApiResponse::err(e))); }
    let dir = state.stacks_dir.join(&name);
    if !dir.exists() { return Ok(Json(ApiResponse::err("Nicht gefunden"))); }
    let out = tokio::process::Command::new("docker").args(["compose", "up", "-d"]).current_dir(&dir).output().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let txt = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(Json(ApiResponse::ok(txt))) } else { Ok(Json(ApiResponse::err(txt))) }
}

async fn agent_stop_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(name): Path<String>) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    let dir = state.stacks_dir.join(&name);
    if !dir.exists() { return Ok(Json(ApiResponse::err("Nicht gefunden"))); }
    let out = tokio::process::Command::new("docker").args(["compose", "down"]).current_dir(&dir).output().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let txt = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(Json(ApiResponse::ok(txt))) } else { Ok(Json(ApiResponse::err(txt))) }
}

async fn agent_redeploy_stack(State(state): State<Arc<AgentState>>, headers: HeaderMap, Path(name): Path<String>) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    let dir = state.stacks_dir.join(&name);
    if !dir.exists() { return Ok(Json(ApiResponse::err("Nicht gefunden"))); }
    let _ = tokio::process::Command::new("docker").args(["compose", "pull"]).current_dir(&dir).output().await;
    let out = tokio::process::Command::new("docker").args(["compose", "up", "-d", "--force-recreate"]).current_dir(&dir).output().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let txt = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(Json(ApiResponse::ok(txt))) } else { Ok(Json(ApiResponse::err(txt))) }
}

// === Docker Registry Login ===

// === Live Stats WebSocket ===

#[derive(Deserialize)]
struct StatsQuery {
    token: Option<String>,
}

async fn agent_stats_live(
    State(state): State<Arc<AgentState>>,
    Query(query): Query<StatsQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let authed = {
        let lock = state.paired_token.read().unwrap();
        match lock.as_ref() {
            None => true,
            Some(stored) => {
                let from_header = headers.get("X-Agent-Token").and_then(|v| v.to_str().ok());
                let from_query = query.token.as_deref();
                from_header == Some(stored.as_str()) || from_query == Some(stored.as_str())
            }
        }
    };
    if !authed {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    let docker = state.docker.clone();
    ws.on_upgrade(move |socket| handle_agent_stats(socket, docker))
}

async fn handle_agent_stats(mut socket: axum::extract::ws::WebSocket, docker: Docker) {
    use futures_util::SinkExt;
    use bollard::container::{ListContainersOptions, StatsOptions};

    loop {
        let mut filters = std::collections::HashMap::new();
        filters.insert("status".to_string(), vec!["running".to_string()]);
        let containers = match docker.list_containers(Some(ListContainersOptions {
            all: true, filters, ..Default::default()
        })).await {
            Ok(c) => c,
            Err(_) => break,
        };

        let mut stats_list = Vec::new();
        for c in &containers {
            let id = match &c.id { Some(id) => id.clone(), None => continue };
            let name = c.names.as_ref().and_then(|n| n.first()).map(|n| n.trim_start_matches('/').to_string()).unwrap_or_default();

            let options = StatsOptions { stream: false, one_shot: true };
            let mut stream = docker.stats(&id, Some(options));
            if let Some(Ok(s)) = futures_lite::StreamExt::next(&mut stream).await {
                let cpu_percent = {
                    let cpu_delta = s.cpu_stats.cpu_usage.total_usage as f64 - s.precpu_stats.cpu_usage.total_usage as f64;
                    let sys_delta = s.cpu_stats.system_cpu_usage.unwrap_or(0) as f64 - s.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
                    let num_cpus = s.cpu_stats.online_cpus.unwrap_or(1) as f64;
                    if sys_delta > 0.0 && cpu_delta >= 0.0 { (cpu_delta / sys_delta) * num_cpus * 100.0 } else { 0.0 }
                };
                let mem_usage = s.memory_stats.usage.unwrap_or(0);
                let mem_limit = s.memory_stats.limit.unwrap_or(1);
                let mem_percent = if mem_limit > 0 { (mem_usage as f64 / mem_limit as f64) * 100.0 } else { 0.0 };
                let (net_rx, net_tx) = s.networks.as_ref().map(|nets| {
                    nets.values().fold((0u64, 0u64), |(rx, tx), n| (rx + n.rx_bytes, tx + n.tx_bytes))
                }).unwrap_or((0, 0));
                let (blk_r, blk_w) = s.blkio_stats.io_service_bytes_recursive.as_ref().map(|entries| {
                    entries.iter().fold((0u64, 0u64), |(r, w), e| {
                        match e.op.as_str() {
                            "read" | "Read" => (r + e.value, w),
                            "write" | "Write" => (r, w + e.value),
                            _ => (r, w),
                        }
                    })
                }).unwrap_or((0, 0));

                stats_list.push(serde_json::json!({
                    "id": id, "name": name,
                    "cpu_percent": (cpu_percent * 100.0).round() / 100.0,
                    "memory_usage": mem_usage, "memory_limit": mem_limit,
                    "memory_percent": (mem_percent * 100.0).round() / 100.0,
                    "network_rx": net_rx, "network_tx": net_tx,
                    "block_read": blk_r, "block_write": blk_w,
                }));
            }
        }

        let snapshot = serde_json::json!({ "containers": stats_list, "timestamp": chrono::Utc::now().timestamp() });
        if socket.send(axum::extract::ws::Message::Text(snapshot.to_string().into())).await.is_err() { break; }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

// === Docker Registry Login ===

#[derive(Debug, Serialize, Deserialize)]
struct RegistryLoginReq { registry: String, username: String, password: String }

async fn agent_docker_login(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Json(req): Json<RegistryLoginReq>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;

    // Ensure docker config dir exists
    std::fs::create_dir_all("/root/.docker").ok();

    let mut child = tokio::process::Command::new("docker")
        .args(["login", &req.registry, "-u", &req.username, "--password-stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(ref mut stdin) = child.stdin {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(req.password.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let output = child.wait_with_output().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if output.status.success() {
        tracing::info!("Docker login successful for {}", req.registry);
        Ok(Json(ApiResponse::ok("Login erfolgreich".into())))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Ok(Json(ApiResponse::err(format!("Login fehlgeschlagen: {}", err.trim()))))
    }
}

async fn agent_docker_logout(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
    Path(registry): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    state.check_auth(&headers)?;
    let _ = tokio::process::Command::new("docker").args(["logout", &registry]).output().await;
    Ok(Json(ApiResponse::ok("Logout erfolgreich".into())))
}

// === Terminal (WebSocket exec) ===

#[derive(Deserialize)]
struct TerminalQuery {
    token: Option<String>,
    shell: Option<String>,
    user: Option<String>,
}

async fn agent_terminal(
    State(state): State<Arc<AgentState>>,
    Path(id): Path<String>,
    Query(query): Query<TerminalQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    // Auth via header OR query param
    let authed = {
        let lock = state.paired_token.read().unwrap();
        match lock.as_ref() {
            None => true,
            Some(stored) => {
                let from_header = headers.get("X-Agent-Token").and_then(|v| v.to_str().ok());
                let from_query = query.token.as_deref();
                from_header == Some(stored.as_str()) || from_query == Some(stored.as_str())
            }
        }
    };
    if !authed {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }

    let shell = query.shell.unwrap_or_else(|| "/bin/sh".to_string());
    let user = query.user;
    ws.on_upgrade(move |socket| agent_handle_terminal(socket, state, id, shell, user))
}

async fn agent_handle_terminal(
    mut socket: WebSocket,
    state: Arc<AgentState>,
    container_id: String,
    shell: String,
    user: Option<String>,
) {
    use futures_lite::StreamExt as FStreamExt;
    use futures_util::SinkExt;
    use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults, ResizeExecOptions};
    use tokio::io::AsyncWriteExt;

    let cmd = vec![shell.as_str()];
    let exec_opts = CreateExecOptions {
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        cmd: Some(cmd),
        user: user.as_deref(),
        ..Default::default()
    };

    let exec_id = match state.docker.create_exec(&container_id, exec_opts).await {
        Ok(r) => r.id,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("\x1b[31mShell nicht verfügbar: {}\x1b[0m\r\n", e).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    let result = match state.docker.start_exec(&exec_id, Some(StartExecOptions { detach: false, tty: true, ..Default::default() })).await {
        Ok(r) => r,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("\x1b[31mExec fehlgeschlagen: {}\x1b[0m\r\n", e).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    match result {
        StartExecResults::Attached { mut output, mut input } => {
            let (mut ws_tx, mut ws_rx) = futures_util::StreamExt::split(socket);

            let docker = state.docker.clone();
            let eid = exec_id.clone();

            // Docker → WS
            let out_handle = tokio::spawn(async move {
                while let Some(Ok(log)) = FStreamExt::next(&mut output).await {
                    let bytes = log.into_bytes();
                    if ws_tx.send(Message::Binary(bytes.into())).await.is_err() { break; }
                }
            });

            // WS → Docker
            let in_handle = tokio::spawn(async move {
                while let Some(Ok(msg)) = futures_util::StreamExt::next(&mut ws_rx).await {
                    match msg {
                        Message::Text(ref text) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                                if val.get("type").and_then(|t| t.as_str()) == Some("resize") {
                                    let cols = val.get("cols").and_then(|c| c.as_u64()).unwrap_or(80) as u16;
                                    let rows = val.get("rows").and_then(|r| r.as_u64()).unwrap_or(24) as u16;
                                    let _ = docker.resize_exec(&eid, ResizeExecOptions { width: cols, height: rows }).await;
                                    continue;
                                }
                            }
                            if input.write_all(text.as_bytes()).await.is_err() { break; }
                        }
                        Message::Binary(data) => {
                            if input.write_all(&data).await.is_err() { break; }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            });

            tokio::select! { _ = out_handle => {}, _ = in_handle => {} }
        }
        StartExecResults::Detached => {}
    }
}

// === Disk Usage ===

#[derive(Debug, Serialize, Deserialize)]
struct DiskUsageInfo { images_size: f64, containers_size: f64, volumes_size: f64, build_cache_size: f64, total_size: f64 }

async fn disk_usage(State(state): State<Arc<AgentState>>, headers: HeaderMap) -> Result<Json<ApiResponse<DiskUsageInfo>>, StatusCode> {
    state.check_auth(&headers)?;
    let df = state.docker.df().await;
    let to_mb = |b: i64| b as f64 / 1_000_000.0;
    match df {
        Ok(d) => {
            let imgs = d.images.as_ref().map(|v| v.iter().map(|i| i.size).sum::<i64>()).unwrap_or(0);
            let ctrs = d.containers.as_ref().map(|v| v.iter().map(|c| c.size_rw.unwrap_or(0)).sum::<i64>()).unwrap_or(0);
            let vols = d.volumes.as_ref().map(|v| v.iter().map(|vol| vol.usage_data.as_ref().map(|u| u.size).unwrap_or(0)).sum::<i64>()).unwrap_or(0);
            let cache = d.build_cache.as_ref().map(|v| v.iter().filter_map(|b| b.size).sum::<i64>()).unwrap_or(0);
            Ok(Json(ApiResponse::ok(DiskUsageInfo { images_size: to_mb(imgs), containers_size: to_mb(ctrs), volumes_size: to_mb(vols), build_cache_size: to_mb(cache), total_size: to_mb(imgs+ctrs+vols+cache) })))
        }
        Err(e) => Ok(Json(ApiResponse::err(e.to_string()))),
    }
}

// === System Info ===

#[derive(Debug, Serialize, Deserialize)]
struct SystemInfoResp {
    hostname: String,
    docker_version: String,
    os: String,
    cpus: i64,
    memory_bytes: i64,
    memory_display: String,
    containers_running: i64,
    containers_stopped: i64,
    containers_paused: i64,
    containers_total: i64,
    images: i64,
    volumes: usize,
    networks: usize,
    status: String,
    server_type: String,
}

async fn system_info(
    State(state): State<Arc<AgentState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<SystemInfoResp>>, StatusCode> {
    state.check_auth(&headers)?;

    let info = state.docker.info().await.ok();
    let version = state.docker.version().await.ok();

    use bollard::volume::ListVolumesOptions;
    use bollard::network::ListNetworksOptions;
    let vol_count = state.docker.list_volumes(Some(ListVolumesOptions::<String> { ..Default::default() })).await
        .map(|r| r.volumes.unwrap_or_default().len()).unwrap_or(0);
    let net_count = state.docker.list_networks(Some(ListNetworksOptions::<String> { ..Default::default() })).await
        .map(|r| r.len()).unwrap_or(0);

    let mem = info.as_ref().and_then(|i| i.mem_total).unwrap_or(0);
    let mem_gb = mem as f64 / 1_073_741_824.0;

    Ok(Json(ApiResponse::ok(SystemInfoResp {
        hostname: state.hostname.clone(),
        docker_version: version.and_then(|v| v.version).unwrap_or_else(|| "unknown".into()),
        os: info.as_ref().and_then(|i| i.operating_system.clone()).unwrap_or_default(),
        cpus: info.as_ref().and_then(|i| i.ncpu).unwrap_or(0),
        memory_bytes: mem,
        memory_display: format!("{:.1} GB", mem_gb),
        containers_running: info.as_ref().and_then(|i| i.containers_running).unwrap_or(0),
        containers_stopped: info.as_ref().and_then(|i| i.containers_stopped).unwrap_or(0),
        containers_paused: info.as_ref().and_then(|i| i.containers_paused).unwrap_or(0),
        containers_total: info.as_ref().and_then(|i| i.containers).unwrap_or(0),
        images: info.as_ref().and_then(|i| i.images).unwrap_or(0),
        volumes: vol_count,
        networks: net_count,
        status: "online".into(),
        server_type: "Standalone".into(),
    })))
}

// === Main ===

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("dockpit_agent=info")
        .init();

    let port = std::env::var("AGENT_PORT")
        .unwrap_or_else(|_| "5522".to_string())
        .parse::<u16>()
        .unwrap_or(5522);

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let docker = Docker::connect_with_socket_defaults()
        .expect("Failed to connect to Docker socket");

    tracing::info!("Hostname: {}", hostname);

    let stacks_dir = std::env::var("AGENT_STACKS_DIR")
        .unwrap_or_else(|_| "/var/docker/container".to_string());
    tracing::info!("Stacks directory: {}", stacks_dir);

    let state = Arc::new(AgentState {
        docker,
        hostname,
        paired_token: RwLock::new(None),
        stacks_dir: std::path::PathBuf::from(stacks_dir),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/pair", post(pair))
        .route("/api/containers", get(list_containers))
        .route("/api/containers/{id}/action", post(container_action))
        .route("/api/containers/{id}/logs", get(container_logs))
        .route("/api/images", get(list_images))
        .route("/api/containers/{id}/check-update", post(check_container_update))
        .route("/api/containers/{id}/recreate", post(recreate_container))
        .route("/api/containers/{id}/terminal", get(agent_terminal))
        .route("/api/stats", get(agent_stats_live))
        .route("/api/docker/login", post(agent_docker_login))
        .route("/api/docker/logout/{registry}", delete(agent_docker_logout))
        .route("/api/images/{id}", delete(remove_image))
        .route("/api/images/prune", post(prune_images))
        .route("/api/volumes", get(list_volumes))
        .route("/api/volumes/prune", post(prune_volumes))
        .route("/api/volumes/{name}", delete(remove_volume))
        .route("/api/networks", get(list_networks))
        .route("/api/networks/prune", post(prune_networks))
        .route("/api/networks/{id}", delete(remove_network))
        .route("/api/disk-usage", get(disk_usage))
        .route("/api/system", get(system_info))
        // Stacks
        .route("/api/stacks", get(agent_list_stacks))
        .route("/api/stacks", post(agent_create_stack))
        .route("/api/stacks/{name}", get(agent_get_stack))
        .route("/api/stacks/{name}", put(agent_update_stack))
        .route("/api/stacks/{name}", delete(agent_delete_stack))
        .route("/api/stacks/{name}/deploy", post(agent_deploy_stack))
        .route("/api/stacks/{name}/stop", post(agent_stop_stack))
        .route("/api/stacks/{name}/redeploy", post(agent_redeploy_stack))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("DockPit Agent starting on {}", addr);
    tracing::info!("Ready - add this agent via the DockPit management UI");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
