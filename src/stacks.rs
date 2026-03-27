use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::models::*;

const COMPOSE_FILES: &[&str] = &[
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];

pub struct StackManager {
    base_dir: PathBuf,
}

impl StackManager {
    pub fn new() -> Self {
        let base_dir = std::env::var("DOCKPIT_STACKS_DIR")
            .unwrap_or_else(|_| "/var/docker/container".to_string());
        Self {
            base_dir: PathBuf::from(base_dir),
        }
    }

    pub async fn list_stacks(&self) -> Result<Vec<StackInfo>, String> {
        let entries =
            std::fs::read_dir(&self.base_dir).map_err(|e| format!("Verzeichnis nicht lesbar: {}", e))?;

        let mut stacks = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if Self::find_compose_file(&path).is_none() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let compose = Self::read_compose_file(&path);
            let services_count = Self::count_services(&compose);
            let (status, running) = self.get_stack_status(&name).await;

            stacks.push(StackInfo {
                name,
                path: path.to_string_lossy().to_string(),
                status,
                services_count,
                running_services: running,
            });
        }

        stacks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(stacks)
    }

    pub async fn get_stack_detail(&self, name: &str) -> Result<StackDetail, String> {
        Self::validate_name(name)?;
        let dir = self.base_dir.join(name);
        if !dir.exists() {
            return Err("Stack nicht gefunden".into());
        }

        let compose_file = Self::find_compose_file(&dir).ok_or("Keine Compose-Datei gefunden")?;
        let compose_content =
            std::fs::read_to_string(&compose_file).map_err(|e| format!("Lesefehler: {}", e))?;
        let services_count = Self::count_services(&compose_content);
        let (status, running) = self.get_stack_status(name).await;
        let containers = self.get_stack_containers(name).await;

        // Read .env
        let env_path = dir.join(".env");
        let env_content = if env_path.exists() {
            std::fs::read_to_string(&env_path).ok()
        } else {
            None
        };

        // Read extra files (everything except compose file, .env, hidden files)
        let compose_name = compose_file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let mut extra_files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname == compose_name
                    || fname == ".env"
                    || fname.starts_with('.')
                    || entry.path().is_dir()
                {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    extra_files.push(StackFile {
                        name: fname,
                        content,
                    });
                }
            }
        }
        extra_files.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(StackDetail {
            name: name.to_string(),
            path: dir.to_string_lossy().to_string(),
            status,
            services_count,
            running_services: running,
            compose_content,
            env_content,
            extra_files,
            containers,
        })
    }

    pub fn create_stack(&self, req: &CreateStackRequest) -> Result<(), String> {
        Self::validate_name(&req.name)?;
        Self::validate_yaml(&req.compose_content)?;

        let dir = self.base_dir.join(&req.name);
        if dir.exists() {
            return Err(format!("Verzeichnis '{}' existiert bereits", req.name));
        }

        std::fs::create_dir_all(&dir).map_err(|e| format!("Verzeichnis erstellen: {}", e))?;

        // Write docker-compose.yml
        std::fs::write(dir.join("docker-compose.yml"), &req.compose_content)
            .map_err(|e| format!("Compose schreiben: {}", e))?;

        // Write .env
        if let Some(ref env_content) = req.env_content {
            if !env_content.trim().is_empty() {
                std::fs::write(dir.join(".env"), env_content)
                    .map_err(|e| format!(".env schreiben: {}", e))?;
            }
        }

        // Write extra files
        if let Some(ref files) = req.extra_files {
            for f in files {
                Self::validate_filename(&f.name)?;
                std::fs::write(dir.join(&f.name), &f.content)
                    .map_err(|e| format!("'{}' schreiben: {}", f.name, e))?;
            }
        }

        Ok(())
    }

    pub fn update_stack(&self, name: &str, req: &UpdateStackRequest) -> Result<(), String> {
        Self::validate_name(name)?;
        Self::validate_yaml(&req.compose_content)?;

        let dir = self.base_dir.join(name);
        if !dir.exists() {
            return Err("Stack nicht gefunden".into());
        }

        // Find and overwrite compose file
        let compose_path = Self::find_compose_file(&dir)
            .unwrap_or_else(|| dir.join("docker-compose.yml"));
        std::fs::write(&compose_path, &req.compose_content)
            .map_err(|e| format!("Compose schreiben: {}", e))?;

        // Handle .env
        let env_path = dir.join(".env");
        match &req.env_content {
            Some(content) if !content.trim().is_empty() => {
                std::fs::write(&env_path, content)
                    .map_err(|e| format!(".env schreiben: {}", e))?;
            }
            _ => {
                // Remove .env if empty or None
                if env_path.exists() {
                    std::fs::remove_file(&env_path).ok();
                }
            }
        }

        // Handle extra files: remove old ones that are not in the new list, write new ones
        let compose_name = compose_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let new_file_names: Vec<String> = req
            .extra_files
            .as_ref()
            .map(|f| f.iter().map(|x| x.name.clone()).collect())
            .unwrap_or_default();

        // Remove files not in the new list
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname == compose_name
                    || fname == ".env"
                    || fname.starts_with('.')
                    || entry.path().is_dir()
                {
                    continue;
                }
                if !new_file_names.contains(&fname) {
                    std::fs::remove_file(entry.path()).ok();
                }
            }
        }

        // Write new/updated extra files
        if let Some(ref files) = req.extra_files {
            for f in files {
                Self::validate_filename(&f.name)?;
                std::fs::write(dir.join(&f.name), &f.content)
                    .map_err(|e| format!("'{}' schreiben: {}", f.name, e))?;
            }
        }

        Ok(())
    }

    pub fn delete_stack(&self, name: &str) -> Result<(), String> {
        Self::validate_name(name)?;
        let dir = self.base_dir.join(name);
        if !dir.exists() {
            return Err("Stack nicht gefunden".into());
        }
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Löschen fehlgeschlagen: {}", e))?;
        Ok(())
    }

    pub async fn deploy_stack(&self, name: &str) -> Result<String, String> {
        let dir = self.stack_dir(name)?;
        Self::run_compose(&dir, &["up", "-d"]).await
    }

    pub async fn stop_stack(&self, name: &str) -> Result<String, String> {
        let dir = self.stack_dir(name)?;
        Self::run_compose(&dir, &["down"]).await
    }

    pub async fn redeploy_stack(&self, name: &str) -> Result<String, String> {
        let dir = self.stack_dir(name)?;
        // Pull latest images first
        let _ = Self::run_compose(&dir, &["pull"]).await;
        // Then recreate
        Self::run_compose(&dir, &["up", "-d", "--force-recreate"]).await
    }

    pub async fn restart_stack(&self, name: &str) -> Result<String, String> {
        let dir = self.stack_dir(name)?;
        Self::run_compose(&dir, &["restart"]).await
    }

    // === Private helpers ===

    pub fn stack_dir_by_name(&self, name: &str) -> Option<PathBuf> {
        let dir = self.base_dir.join(name);
        if dir.exists() { Some(dir) } else { None }
    }

    fn stack_dir(&self, name: &str) -> Result<PathBuf, String> {
        Self::validate_name(name)?;
        let dir = self.base_dir.join(name);
        if !dir.exists() {
            return Err("Stack nicht gefunden".into());
        }
        Ok(dir)
    }

    fn validate_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("Name darf nicht leer sein".into());
        }
        if name.contains("..")
            || name.contains('/')
            || name.contains('\\')
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(
                "Ungültiger Name. Nur Buchstaben, Zahlen, Bindestriche und Unterstriche.".into(),
            );
        }
        Ok(())
    }

    fn validate_filename(name: &str) -> Result<(), String> {
        if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
            return Err(format!("Ungültiger Dateiname: '{}'", name));
        }
        Ok(())
    }

    fn validate_yaml(content: &str) -> Result<(), String> {
        // Check for tab characters
        if content.contains('\t') {
            return Err(
                "YAML-Fehler: Tabs sind nicht erlaubt in YAML. Verwende Leerzeichen.".into(),
            );
        }
        serde_yaml::from_str::<serde_yaml::Value>(content)
            .map_err(|e| format!("YAML-Fehler: {}", e))?;
        Ok(())
    }

    fn find_compose_file(dir: &Path) -> Option<PathBuf> {
        COMPOSE_FILES
            .iter()
            .map(|name| dir.join(name))
            .find(|p| p.exists())
    }

    fn read_compose_file(dir: &Path) -> String {
        Self::find_compose_file(dir)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default()
    }

    fn count_services(compose_content: &str) -> usize {
        serde_yaml::from_str::<serde_yaml::Value>(compose_content)
            .ok()
            .and_then(|v| v.get("services")?.as_mapping().map(|m| m.len()))
            .unwrap_or(0)
    }

    pub async fn get_stack_containers(&self, name: &str) -> Vec<crate::models::ContainerInfo> {
        // Docker compose converts project names to lowercase
        let project = name.to_lowercase();
        let output = Command::new("docker")
            .args(["ps", "-a", "--filter", &format!("label=com.docker.compose.project={}", project),
                   "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Status}}\t{{.Ports}}\t{{.Label \"com.docker.compose.project\"}}"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|line| {
                        let parts: Vec<&str> = line.splitn(7, '\t').collect();
                        crate::models::ContainerInfo {
                            id: parts.first().unwrap_or(&"").to_string(),
                            name: parts.get(1).unwrap_or(&"").to_string(),
                            image: parts.get(2).unwrap_or(&"").to_string(),
                            state: parts.get(3).unwrap_or(&"").to_string(),
                            status: parts.get(4).unwrap_or(&"").to_string(),
                            ports: Self::parse_ports(parts.get(5).unwrap_or(&"")),
                            created: 0,
                            environment_id: None,
                            ip_address: None,
                            stack_name: parts.get(6).map(|s| s.to_string()).filter(|s| !s.is_empty()),
                        }
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    fn parse_ports(ports_str: &str) -> Vec<crate::models::PortMapping> {
        // Parse docker ps port format like "0.0.0.0:8080->80/tcp"
        ports_str.split(", ")
            .filter(|s| !s.is_empty() && s.contains("->"))
            .filter_map(|p| {
                let arrow = p.find("->")?;
                let host_part = &p[..arrow];
                let container_part = &p[arrow + 2..];
                let public_port: u16 = host_part.rsplit(':').next()?.parse().ok()?;
                let private_str = container_part.split('/').next()?;
                let private_port: u16 = private_str.parse().ok()?;
                let port_type = container_part.split('/').nth(1).unwrap_or("tcp").to_string();
                Some(crate::models::PortMapping { private_port, public_port: Some(public_port), port_type })
            })
            .collect()
    }

    async fn get_stack_status(&self, name: &str) -> (String, usize) {
        let containers = self.get_stack_containers(name).await;
        if containers.is_empty() {
            return ("stopped".into(), 0);
        }
        let running = containers.iter().filter(|c| c.state == "running").count();
        let status = if running == containers.len() {
            "running"
        } else if running > 0 {
            "partial"
        } else {
            "stopped"
        };
        (status.into(), running)
    }

    async fn run_compose(dir: &Path, args: &[&str]) -> Result<String, String> {
        let mut cmd_args = vec!["compose"];
        cmd_args.extend_from_slice(args);

        let output = Command::new("docker")
            .args(&cmd_args)
            .current_dir(dir)
            .output()
            .await
            .map_err(|e| format!("Ausführung fehlgeschlagen: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}{}", stdout, stderr);

        if output.status.success() {
            Ok(combined)
        } else {
            Err(format!("docker compose Fehler:\n{}", combined))
        }
    }
}
