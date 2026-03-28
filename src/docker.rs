use bollard::Docker;
use bollard::container::{ListContainersOptions, LogsOptions, StartContainerOptions, StopContainerOptions, RemoveContainerOptions, StatsOptions};
use bollard::system::EventsOptions;
use bollard::image::{ListImagesOptions, RemoveImageOptions};
use bollard::volume::ListVolumesOptions;
use bollard::network::ListNetworksOptions;
use futures_lite::StreamExt;
use std::collections::HashMap;

use crate::models::*;

pub struct DockerClient {
    docker: Docker,
}

impl DockerClient {
    pub fn clone_inner(&self) -> Docker {
        self.docker.clone()
    }

    pub fn from_docker(docker: Docker) -> Self {
        Self { docker }
    }

    pub fn new() -> Result<Self, bollard::errors::Error> {
        let docker = Docker::connect_with_socket_defaults()?;
        Ok(Self { docker })
    }

    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>, bollard::errors::Error> {
        let mut filters = HashMap::new();
        filters.insert("status", vec!["running", "exited", "paused", "created", "restarting", "dead"]);

        let options = ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        };

        let containers = self.docker.list_containers(Some(options)).await?;

        Ok(containers
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

                // Extract IP from first network
                let ip_address = c.network_settings
                    .as_ref()
                    .and_then(|ns| ns.networks.as_ref())
                    .and_then(|nets| nets.values().next())
                    .and_then(|net| net.ip_address.clone())
                    .filter(|ip| !ip.is_empty());

                // Extract stack name from compose label
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
            .collect())
    }

    pub async fn start_container(&self, id: &str) -> Result<(), bollard::errors::Error> {
        self.docker.start_container(id, None::<StartContainerOptions<String>>).await
    }

    pub async fn stop_container(&self, id: &str) -> Result<(), bollard::errors::Error> {
        self.docker
            .stop_container(id, Some(StopContainerOptions { t: 10 }))
            .await
    }

    pub async fn restart_container(&self, id: &str) -> Result<(), bollard::errors::Error> {
        self.docker.restart_container(id, Some(bollard::container::RestartContainerOptions { t: 10 })).await
    }

    pub async fn remove_container(&self, id: &str) -> Result<(), bollard::errors::Error> {
        self.docker
            .remove_container(
                id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
    }

    pub async fn inspect_container(&self, id: &str) -> Result<bollard::models::ContainerInspectResponse, bollard::errors::Error> {
        self.docker.inspect_container(id, None).await
    }

    pub async fn inspect_image(&self, name: &str) -> Result<bollard::models::ImageInspect, bollard::errors::Error> {
        self.docker.inspect_image(name).await
    }

    /// Check if image has update available by comparing local image ID with the
    /// remote image config digest from the registry. No image pull needed.
    /// Returns (outdated: bool, local_id, remote_config_digest)
    pub async fn check_image_update(&self, image: &str) -> Result<(bool, String, String), String> {
        let local = self.docker.inspect_image(image).await
            .map_err(|e| format!("Image not found: {}", e))?;
        let local_id = local.id.clone().unwrap_or_default(); // sha256:... (config digest)
        let local_digests = local.repo_digests.unwrap_or_default();

        if local_digests.is_empty() {
            // Locally built image — no registry to compare against
            return Ok((false, local_id.clone(), local_id));
        }

        let (registry, repo, tag) = parse_image_ref(image);

        // Fetch remote config digest from registry (resolves manifest list → platform manifest → config)
        let remote_config_digest = match fetch_remote_config_digest(&registry, &repo, &tag).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Registry check failed for {}: {}", image, e);
                return Ok((false, local_id.clone(), local_id));
            }
        };

        // Compare local image ID (config digest) with remote config digest
        // Both are in format "sha256:..."
        let local_clean = local_id.trim_start_matches("sha256:").to_string();
        let remote_clean = remote_config_digest.trim_start_matches("sha256:").to_string();

        let outdated = !remote_clean.is_empty() && local_clean != remote_clean;
        Ok((outdated, local_id, remote_config_digest))
    }

    /// Recreate a container: pull latest image, stop old, create new with same config, start, remove old
    pub async fn recreate_container(&self, id: &str) -> Result<String, String> {
        use bollard::container::CreateContainerOptions;

        // 1. Inspect
        let inspect = self.docker.inspect_container(id, None).await
            .map_err(|e| format!("Inspect: {}", e))?;
        let config = inspect.config.ok_or("Keine Config")?;
        let image = config.image.clone().unwrap_or_default();
        let name = inspect.name.unwrap_or_default().trim_start_matches('/').to_string();
        let host_config = inspect.host_config;

        // 2. Pull latest image
        self.pull_image(&image).await.map_err(|e| format!("Pull: {}", e))?;

        // 3. Stop and remove old container
        let _ = self.docker.stop_container(id, Some(bollard::container::StopContainerOptions { t: 10 })).await;
        self.docker.remove_container(id, Some(bollard::container::RemoveContainerOptions { force: true, ..Default::default() })).await
            .map_err(|e| format!("Remove: {}", e))?;

        // 4. Create new container with same config
        let create_opts = CreateContainerOptions { name: name.clone(), ..Default::default() };
        let networking_config = inspect.network_settings
            .and_then(|ns| ns.networks)
            .map(|nets| {
                let endpoints: HashMap<String, bollard::models::EndpointSettings> = nets.into_iter().map(|(k, v)| {
                    (k, bollard::models::EndpointSettings {
                        aliases: v.aliases,
                        network_id: v.network_id,
                        ..Default::default()
                    })
                }).collect();
                bollard::container::NetworkingConfig { endpoints_config: endpoints }
            });

        let body = bollard::container::Config {
            image: Some(image.clone()),
            hostname: config.hostname,
            domainname: config.domainname,
            user: config.user,
            env: config.env,
            cmd: config.cmd,
            entrypoint: config.entrypoint,
            working_dir: config.working_dir,
            labels: config.labels,
            exposed_ports: config.exposed_ports,
            volumes: config.volumes,
            tty: config.tty,
            open_stdin: config.open_stdin,
            stdin_once: config.stdin_once,
            attach_stdin: config.attach_stdin,
            attach_stdout: config.attach_stdout,
            attach_stderr: config.attach_stderr,
            stop_signal: config.stop_signal,
            healthcheck: config.healthcheck,
            host_config: host_config,
            networking_config,
            ..Default::default()
        };

        let created = self.docker.create_container(Some(create_opts), body).await
            .map_err(|e| format!("Create: {}", e))?;

        // 5. Start
        self.docker.start_container(&created.id, None::<bollard::container::StartContainerOptions<String>>).await
            .map_err(|e| format!("Start: {}", e))?;

        Ok(format!("Container '{}' neu erstellt mit aktuellem Image", name))
    }

    pub async fn create_exec(
        &self,
        container_id: &str,
        cmd: Vec<&str>,
        user: Option<&str>,
    ) -> Result<String, bollard::errors::Error> {
        use bollard::exec::CreateExecOptions;
        let options = CreateExecOptions {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(true),
            cmd: Some(cmd),
            user,
            ..Default::default()
        };
        let exec = self.docker.create_exec(container_id, options).await?;
        Ok(exec.id)
    }

    pub async fn start_exec(
        &self,
        exec_id: &str,
    ) -> Result<bollard::exec::StartExecResults, bollard::errors::Error> {
        use bollard::exec::StartExecOptions;
        self.docker.start_exec(exec_id, Some(StartExecOptions { detach: false, tty: true, ..Default::default() })).await
    }

    pub async fn resize_exec(
        &self,
        exec_id: &str,
        width: u16,
        height: u16,
    ) -> Result<(), bollard::errors::Error> {
        use bollard::exec::ResizeExecOptions;
        self.docker.resize_exec(exec_id, ResizeExecOptions { width, height }).await
    }

    pub async fn container_logs(&self, id: &str, tail: usize) -> Result<String, bollard::errors::Error> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail.to_string(),
            timestamps: true,
            ..Default::default()
        };

        let mut stream = self.docker.logs(id, Some(options));
        let mut logs = String::new();

        while let Some(Ok(log)) = stream.next().await {
            logs.push_str(&log.to_string());
        }

        Ok(logs)
    }

    pub async fn list_images(&self) -> Result<Vec<ImageInfo>, bollard::errors::Error> {
        let options = ListImagesOptions::<String> {
            all: false,
            ..Default::default()
        };

        let images = self.docker.list_images(Some(options)).await?;

        // Get all image IDs used by containers
        let containers = self.docker.list_containers(Some(ListContainersOptions::<&str> {
            all: true, ..Default::default()
        })).await.unwrap_or_default();

        let used_ids: std::collections::HashSet<String> = containers.iter()
            .filter_map(|c| c.image_id.clone())
            .collect();

        Ok(images
            .into_iter()
            .map(|img| {
                let tags = img.repo_tags;
                let size_mb = img.size as f64 / 1_000_000.0;
                let in_use = used_ids.contains(&img.id);

                ImageInfo {
                    id: img.id[..std::cmp::min(19, img.id.len())].to_string(),
                    tags,
                    size: size_mb,
                    created: img.created,
                    in_use,
                }
            })
            .collect())
    }

    pub async fn pull_image(&self, image: &str) -> Result<(), bollard::errors::Error> {
        use bollard::image::CreateImageOptions;

        let (repo, tag) = if let Some((r, t)) = image.split_once(':') {
            (r.to_string(), t.to_string())
        } else {
            (image.to_string(), "latest".to_string())
        };

        let options = CreateImageOptions {
            from_image: repo,
            tag,
            ..Default::default()
        };

        let mut stream = self.docker.create_image(Some(options), None, None);
        while let Some(result) = stream.next().await {
            result?;
        }

        Ok(())
    }

    pub async fn prune_images(&self) -> Result<String, bollard::errors::Error> {
        let result = self.docker.prune_images::<String>(None).await?;
        let deleted = result.images_deleted.map(|v| v.len()).unwrap_or(0);
        let space = result.space_reclaimed.unwrap_or(0);
        Ok(format!("{} Images gelöscht, {:.1} MB freigegeben", deleted, space as f64 / 1_000_000.0))
    }

    pub async fn remove_image(&self, id: &str) -> Result<(), bollard::errors::Error> {
        self.remove_image_force(id, true).await
    }

    pub async fn remove_image_force(&self, id: &str, force: bool) -> Result<(), bollard::errors::Error> {
        self.docker
            .remove_image(
                id,
                Some(RemoveImageOptions {
                    force,
                    ..Default::default()
                }),
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn list_volumes(&self) -> Result<Vec<VolumeInfo>, bollard::errors::Error> {
        let result = self.docker.list_volumes(Some(ListVolumesOptions::<String> {
            ..Default::default()
        })).await?;

        // Get volumes in use by containers
        let containers = self.docker.list_containers(Some(ListContainersOptions::<&str> {
            all: true, ..Default::default()
        })).await.unwrap_or_default();

        let used_volumes: std::collections::HashSet<String> = containers.iter()
            .filter_map(|c| c.mounts.as_ref())
            .flat_map(|mounts| mounts.iter())
            .filter_map(|m| m.name.clone())
            .collect();

        Ok(result
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|v| {
                let in_use = used_volumes.contains(&v.name);
                VolumeInfo {
                    name: v.name,
                    driver: v.driver,
                    mountpoint: v.mountpoint,
                    created: v.created_at,
                    in_use,
                }
            })
            .collect())
    }

    pub async fn remove_volume(&self, name: &str, force: bool) -> Result<(), bollard::errors::Error> {
        use bollard::volume::RemoveVolumeOptions;
        self.docker.remove_volume(name, Some(RemoveVolumeOptions { force })).await
    }

    pub async fn prune_volumes(&self) -> Result<String, bollard::errors::Error> {
        let result = self.docker.prune_volumes::<String>(None).await?;
        let deleted = result.volumes_deleted.map(|v| v.len()).unwrap_or(0);
        let space = result.space_reclaimed.unwrap_or(0);
        Ok(format!("{} Volumes gelöscht, {:.1} MB freigegeben", deleted, space as f64 / 1_000_000.0))
    }

    pub async fn list_networks(&self) -> Result<Vec<NetworkInfo>, bollard::errors::Error> {
        let networks = self.docker.list_networks(Some(ListNetworksOptions::<String> {
            ..Default::default()
        })).await?;

        // Get used network IDs from containers
        let containers = self.docker.list_containers(Some(ListContainersOptions::<&str> {
            all: true, ..Default::default()
        })).await.unwrap_or_default();

        let mut network_usage: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for c in &containers {
            if let Some(ns) = c.network_settings.as_ref().and_then(|s| s.networks.as_ref()) {
                for (net_name, _) in ns {
                    *network_usage.entry(net_name.clone()).or_insert(0) += 1;
                }
            }
        }

        Ok(networks
            .into_iter()
            .map(|n| {
                let name = n.name.unwrap_or_default();
                let containers_count = network_usage.get(&name).copied().unwrap_or(0);
                NetworkInfo {
                    id: n.id.unwrap_or_default(),
                    name,
                    driver: n.driver.unwrap_or_default(),
                    scope: n.scope.unwrap_or_default(),
                    in_use: containers_count > 0,
                    containers_count,
                }
            })
            .collect())
    }

    pub async fn remove_network(&self, id: &str) -> Result<(), bollard::errors::Error> {
        self.docker.remove_network(id).await
    }

    pub async fn prune_networks(&self) -> Result<String, bollard::errors::Error> {
        let result = self.docker.prune_networks::<String>(None).await?;
        let deleted = result.networks_deleted.map(|v| v.len()).unwrap_or(0);
        Ok(format!("{} Netzwerke gelöscht", deleted))
    }

    pub async fn get_disk_usage(&self) -> DiskUsageInfo {
        let df = self.docker.df().await;
        match df {
            Ok(d) => {
                let imgs = d.images.as_ref().map(|v| v.iter().map(|i| i.size).sum::<i64>()).unwrap_or(0);
                let ctrs = d.containers.as_ref().map(|v| v.iter().map(|c| c.size_rw.unwrap_or(0)).sum::<i64>()).unwrap_or(0);
                let vols = d.volumes.as_ref().map(|v| v.iter().map(|vol| vol.usage_data.as_ref().map(|u| u.size).unwrap_or(0)).sum::<i64>()).unwrap_or(0);
                let cache = d.build_cache.as_ref().map(|v| v.iter().filter_map(|b| b.size).sum::<i64>()).unwrap_or(0);
                let to_mb = |b: i64| b as f64 / 1_000_000.0;
                DiskUsageInfo {
                    images_size: to_mb(imgs),
                    containers_size: to_mb(ctrs),
                    volumes_size: to_mb(vols),
                    build_cache_size: to_mb(cache),
                    total_size: to_mb(imgs + ctrs + vols + cache),
                }
            }
            Err(_) => DiskUsageInfo { images_size: 0.0, containers_size: 0.0, volumes_size: 0.0, build_cache_size: 0.0, total_size: 0.0 },
        }
    }

    pub async fn get_system_info(&self) -> SystemInfo {
        let info = self.docker.info().await.ok();
        let version = self.docker.version().await.ok();
        let volumes = self.list_volumes().await.unwrap_or_default();
        let networks = self.list_networks().await.unwrap_or_default();

        let mem_bytes = info.as_ref().and_then(|i| i.mem_total).unwrap_or(0);
        let mem_gb = mem_bytes as f64 / 1_073_741_824.0;

        SystemInfo {
            hostname: info.as_ref().and_then(|i| i.name.clone()).unwrap_or_else(|| "unknown".into()),
            docker_version: version.and_then(|v| v.version).unwrap_or_else(|| "unknown".into()),
            os: info.as_ref().and_then(|i| i.operating_system.clone()).unwrap_or_default(),
            cpus: info.as_ref().and_then(|i| i.ncpu).unwrap_or(0),
            memory_bytes: mem_bytes,
            memory_display: format!("{:.1} GB", mem_gb),
            containers_running: info.as_ref().and_then(|i| i.containers_running).unwrap_or(0),
            containers_stopped: info.as_ref().and_then(|i| i.containers_stopped).unwrap_or(0),
            containers_paused: info.as_ref().and_then(|i| i.containers_paused).unwrap_or(0),
            containers_total: info.as_ref().and_then(|i| i.containers).unwrap_or(0),
            images: info.as_ref().and_then(|i| i.images).unwrap_or(0),
            volumes: volumes.len(),
            networks: networks.len(),
            status: "online".into(),
            server_type: "Standalone".into(),
        }
    }

    pub async fn get_recent_events(&self, since_secs: i64) -> Vec<ContainerEvent> {
        let since = chrono::Utc::now().timestamp() - since_secs;
        let until = chrono::Utc::now().timestamp();

        let mut filters = HashMap::new();
        filters.insert("type".to_string(), vec!["container".to_string()]);
        // Only relevant events — no noise from attach, connect, disconnect, exec, etc.
        filters.insert("event".to_string(), vec![
            "start".to_string(), "stop".to_string(), "die".to_string(),
            "kill".to_string(), "restart".to_string(), "oom".to_string(),
            "destroy".to_string(), "health_status".to_string(),
        ]);

        let options = EventsOptions {
            since: Some(since.to_string()),
            until: Some(until.to_string()),
            filters,
        };

        let mut stream = self.docker.events(Some(options));
        let mut events = Vec::new();

        while let Some(Ok(event)) = stream.next().await {
            let action = event.action.unwrap_or_default();
            let actor = event.actor.unwrap_or_default();
            let container_id = actor.id.unwrap_or_default();
            let container_name = actor.attributes.as_ref()
                .and_then(|a| a.get("name"))
                .cloned()
                .unwrap_or_default();
            let image = actor.attributes.as_ref()
                .and_then(|a| a.get("image"))
                .cloned();
            let ts = event.time.unwrap_or(0);
            let timestamp = chrono::DateTime::from_timestamp(ts, 0)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();

            events.push(ContainerEvent {
                id: None,
                env_id: String::new(), // filled by caller
                container_id: Some(container_id),
                container_name: Some(container_name),
                event_type: "container".into(),
                event_action: action,
                details: image,
                timestamp,
            });
        }

        events
    }

    pub async fn get_all_container_stats(&self) -> Vec<ContainerStats> {
        let containers = match self.list_containers().await {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let running: Vec<_> = containers.into_iter().filter(|c| c.state == "running").collect();
        let mut stats = Vec::with_capacity(running.len());

        for c in &running {
            let options = StatsOptions { stream: false, one_shot: true };
            let mut stream = self.docker.stats(&c.id, Some(options));
            if let Some(Ok(s)) = stream.next().await {
                let cpu_percent = {
                    let cpu_delta = s.cpu_stats.cpu_usage.total_usage as f64
                        - s.precpu_stats.cpu_usage.total_usage as f64;
                    let sys_delta = s.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
                        - s.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
                    let num_cpus = s.cpu_stats.online_cpus.unwrap_or(1) as f64;
                    if sys_delta > 0.0 && cpu_delta >= 0.0 {
                        (cpu_delta / sys_delta) * num_cpus * 100.0
                    } else {
                        0.0
                    }
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

                stats.push(ContainerStats {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    cpu_percent: (cpu_percent * 100.0).round() / 100.0,
                    memory_usage: mem_usage,
                    memory_limit: mem_limit,
                    memory_percent: (mem_percent * 100.0).round() / 100.0,
                    network_rx: net_rx,
                    network_tx: net_tx,
                    block_read: blk_r,
                    block_write: blk_w,
                });
            }
        }

        stats
    }

    pub async fn get_dashboard_stats(&self) -> DashboardStats {
        let containers = self.list_containers().await.unwrap_or_default();
        let images = self.list_images().await.unwrap_or_default();
        let volumes = self.list_volumes().await.unwrap_or_default();
        let networks = self.list_networks().await.unwrap_or_default();

        let running = containers.iter().filter(|c| c.state == "running").count();
        let stopped = containers.iter().filter(|c| c.state != "running").count();

        DashboardStats {
            containers_running: running,
            containers_stopped: stopped,
            containers_total: containers.len(),
            images_total: images.len(),
            volumes_total: volumes.len(),
            networks_total: networks.len(),
            environments: vec![],
        }
    }
}

/// Parse "registry/repo:tag" into (registry, repo, tag).
/// Handles Docker Hub shorthand (e.g. "nginx:latest" → "registry-1.docker.io", "library/nginx", "latest")
/// and third-party registries (e.g. "ghcr.io/user/repo:tag").
fn parse_image_ref(image: &str) -> (String, String, String) {
    let (name, tag) = if let Some((n, t)) = image.rsplit_once(':') {
        // Check that the part after : is a tag, not a port
        if t.contains('/') { (image, "latest") } else { (n, t) }
    } else {
        (image, "latest")
    };

    let parts: Vec<&str> = name.splitn(2, '/').collect();
    if parts.len() == 1 {
        // "nginx" → Docker Hub library
        ("registry-1.docker.io".into(), format!("library/{}", parts[0]), tag.into())
    } else if parts[0].contains('.') || parts[0].contains(':') {
        // "ghcr.io/user/repo" or "registry.example.com:5000/repo"
        let registry = parts[0].to_string();
        let repo = parts[1].to_string();
        (registry, repo, tag.into())
    } else {
        // "user/repo" → Docker Hub
        ("registry-1.docker.io".into(), name.to_string(), tag.into())
    }
}

/// Fetch the CONFIG digest of a remote image. This resolves:
/// 1. Tag → OCI Index / Manifest List
/// 2. Find linux/amd64 manifest
/// 3. Fetch that manifest → extract config.digest
/// The config digest IS the image ID that Docker uses locally.
async fn fetch_remote_config_digest(registry: &str, repo: &str, tag: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    let token = match get_registry_token(&client, registry, repo, true).await {
        Ok(t) => t,
        Err(_) => get_registry_token(&client, registry, repo, false).await.unwrap_or_default(),
    };

    let auth_header = if !token.is_empty() { format!("Bearer {}", token) } else { String::new() };

    // Step 1: GET manifest list / OCI index
    let url = format!("https://{}/v2/{}/manifests/{}", registry, repo, tag);
    let mut req = client.get(&url)
        .header("Accept", "application/vnd.oci.image.index.v1+json")
        .header("Accept", "application/vnd.docker.distribution.manifest.list.v2+json")
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
        .header("Accept", "application/vnd.oci.image.manifest.v1+json");
    if !auth_header.is_empty() {
        req = req.header("Authorization", &auth_header);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Registry returned {} for {}", resp.status(), url));
    }

    let content_type = resp.headers().get("content-type")
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    // If it's a manifest list / OCI index → find the amd64 manifest
    if content_type.contains("manifest.list") || content_type.contains("image.index") {
        let manifests = body["manifests"].as_array()
            .ok_or("No manifests in index")?;

        // Find linux/amd64 image manifest (skip attestations)
        let amd64 = manifests.iter().find(|m| {
            let platform = &m["platform"];
            let arch = platform["architecture"].as_str().unwrap_or("");
            let os = platform["os"].as_str().unwrap_or("");
            let media_type = m["mediaType"].as_str().unwrap_or("");
            arch == "amd64" && os == "linux" && !media_type.contains("attestation")
        }).ok_or("No linux/amd64 manifest found")?;

        let manifest_digest = amd64["digest"].as_str().ok_or("No digest in manifest entry")?;

        // Step 2: Fetch the platform-specific manifest
        let manifest_url = format!("https://{}/v2/{}/manifests/{}", registry, repo, manifest_digest);
        let mut req = client.get(&manifest_url)
            .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
            .header("Accept", "application/vnd.oci.image.manifest.v1+json");
        if !auth_header.is_empty() {
            req = req.header("Authorization", &auth_header);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Registry returned {} for manifest", resp.status()));
        }
        let manifest: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

        // The config digest IS the image ID
        manifest["config"]["digest"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No config digest in manifest".into())
    } else {
        // Already a single manifest — extract config digest directly
        body["config"]["digest"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No config digest in manifest".into())
    }
}

/// Fetch the digest of an image tag from a container registry WITHOUT pulling.
/// Uses Docker Distribution HTTP API v2. Supports authenticated registries
/// by reading credentials from Docker config (~/.docker/config.json).
async fn fetch_registry_digest(registry: &str, repo: &str, tag: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    // Try authenticated first, fall back to anonymous
    let token = match get_registry_token(&client, registry, repo, true).await {
        Ok(t) => t,
        Err(_) => get_registry_token(&client, registry, repo, false).await.unwrap_or_default(),
    };

    let url = format!("https://{}/v2/{}/manifests/{}", registry, repo, tag);
    let mut req = client.head(&url)
        .header("Accept", "application/vnd.oci.image.index.v1+json")
        .header("Accept", "application/vnd.docker.distribution.manifest.list.v2+json")
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json");

    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Unauthorized — registry credentials may be missing".into());
    }
    if !resp.status().is_success() {
        return Err(format!("Registry returned {}", resp.status()));
    }

    resp.headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| "No digest in response".into())
}

/// Read Docker credentials from docker config.json
fn read_docker_credentials(registry: &str) -> Option<(String, String)> {
    // Try multiple config locations
    let paths = [
        std::path::PathBuf::from("/root/.docker/config.json"),
        std::env::var("DOCKER_CONFIG").ok()
            .map(|p| std::path::PathBuf::from(p).join("config.json"))
            .unwrap_or_default(),
        dirs_next::home_dir()
            .map(|h| h.join(".docker/config.json"))
            .unwrap_or_default(),
    ];

    let content = paths.iter()
        .filter(|p| p.as_os_str().len() > 0)
        .find_map(|p| std::fs::read_to_string(p).ok())?;

    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    let auths = config.get("auths")?.as_object()?;

    // Try exact match, then common aliases
    let auth_entry = auths.get(registry)
        .or_else(|| auths.get(&format!("https://{}", registry)))
        .or_else(|| {
            if registry.contains("docker.io") || registry.contains("registry-1") {
                auths.get("https://index.docker.io/v1/")
                    .or_else(|| auths.get("https://index.docker.io/v2/"))
                    .or_else(|| auths.get("docker.io"))
                    .or_else(|| auths.get("registry-1.docker.io"))
            } else {
                // Try partial match for custom registries
                auths.iter().find(|(k, _)| k.contains(registry)).map(|(_, v)| v)
            }
        })?;

    let auth_b64 = auth_entry.get("auth")?.as_str()?;
    let decoded = String::from_utf8(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, auth_b64).ok()?
    ).ok()?;

    let (user, pass) = decoded.split_once(':')?;
    Some((user.to_string(), pass.to_string()))
}

/// Get a bearer token for the registry. If `use_creds` is true, tries authenticated token.
async fn get_registry_token(client: &reqwest::Client, registry: &str, repo: &str, use_creds: bool) -> Result<String, String> {
    let creds = if use_creds { read_docker_credentials(registry) } else { None };

    if registry.contains("docker.io") || registry.contains("registry-1") {
        let url = format!("https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull", repo);
        let mut req = client.get(&url);
        if let Some((user, pass)) = &creds {
            req = req.basic_auth(user, Some(pass));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let token = data["token"].as_str().unwrap_or("").to_string();
        if token.is_empty() { return Err("No token".into()); }
        return Ok(token);
    }

    if registry.contains("ghcr.io") {
        let url = format!("https://ghcr.io/token?service=ghcr.io&scope=repository:{}:pull", repo);
        let mut req = client.get(&url);
        if let Some((user, pass)) = &creds {
            req = req.basic_auth(user, Some(pass));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let token = data["token"].as_str().unwrap_or("").to_string();
        if token.is_empty() { return Err("No token".into()); }
        return Ok(token);
    }

    // Generic registry — try basic auth directly
    if let Some((user, pass)) = creds {
        return Ok(format!("Basic {}", base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{}:{}", user, pass)
        )));
    }

    Ok(String::new())
}
