mod auth;
mod db;
mod docker;
mod handlers;
mod models;
mod stacks;

use axum::{
    Router,
    http::{header, StatusCode, Uri},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
};
use rust_embed::Embed;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use handlers::AppState;
use db::Database;

#[derive(Embed)]
#[folder = "frontend/build/"]
struct FrontendAssets;

fn serve_file(path: &str) -> Option<Response> {
    FrontendAssets::get(path).map(|file| {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache_control = if path.contains("/_app/") || path.contains("/immutable/") {
            "public, max-age=31536000, immutable"
        } else if path.ends_with(".svg") || path.ends_with(".png") || path.ends_with(".ico") || path.ends_with(".woff2") {
            "public, max-age=86400"
        } else if path.ends_with(".html") {
            "no-cache"
        } else {
            "public, max-age=3600"
        };
        let mut resp = (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime.as_ref().to_string()),
                (header::CACHE_CONTROL, cache_control.to_string()),
            ],
            file.data.to_vec(),
        ).into_response();
        if path.ends_with(".html") || path.is_empty() {
            let hdrs = resp.headers_mut();
            hdrs.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
            hdrs.insert("X-Frame-Options", "SAMEORIGIN".parse().unwrap());
            hdrs.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
            hdrs.insert("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ws: wss:; font-src 'self'; frame-src 'self'".parse().unwrap());
        }
        resp
    })
}

async fn serve_frontend(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Exact file match (JS, CSS, SVG, etc.)
    if let Some(resp) = serve_file(path) {
        return resp;
    }

    // Try path.html (e.g. /login -> login.html)
    if !path.is_empty() {
        if let Some(resp) = serve_file(&format!("{}.html", path)) {
            return resp;
        }
        // Try path/index.html
        if let Some(resp) = serve_file(&format!("{}/index.html", path)) {
            return resp;
        }
    }

    // SvelteKit fallback for dynamic routes
    if let Some(resp) = serve_file("200.html") {
        return resp;
    }

    // Final fallback to index.html
    match FrontendAssets::get("index.html") {
        Some(file) => {
            let mut resp = Html(String::from_utf8_lossy(&file.data).to_string()).into_response();
            let hdrs = resp.headers_mut();
            hdrs.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
            hdrs.insert("X-Frame-Options", "SAMEORIGIN".parse().unwrap());
            hdrs.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
            hdrs.insert("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ws: wss:; font-src 'self'; frame-src 'self'".parse().unwrap());
            resp
        },
        None => (StatusCode::NOT_FOUND, "Frontend not found").into_response(),
    }
}

async fn restore_docker_logins(db: &Database) {
    let logins = db.get_all_registry_credentials();
    if logins.is_empty() { return; }

    tracing::info!("Restoring {} Docker registry login(s)...", logins.len());

    for (registry, username, password) in &logins {
        let mut child = match tokio::process::Command::new("docker")
            .args(["login", registry, "-u", username, "--password-stdin"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => { tracing::warn!("Docker login spawn failed for {}: {}", registry, e); continue; }
        };

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(password.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }

        match child.wait().await {
            Ok(s) if s.success() => tracing::info!("Registry login restored: {}", registry),
            Ok(_) => tracing::warn!("Registry login failed for {}", registry),
            Err(e) => tracing::warn!("Registry login error for {}: {}", registry, e),
        }
    }

    // Propagate logins to all remote agents
    let envs = db.get_environments();
    let remote_envs: Vec<_> = envs.into_iter().filter(|e| !e.is_local).collect();
    if remote_envs.is_empty() { return; }

    tracing::info!("Propagating {} registry login(s) to {} remote agent(s)...", logins.len(), remote_envs.len());
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(e) => { tracing::warn!("Failed to create HTTP client: {}", e); return; }
    };

    for env in &remote_envs {
        let token = env.agent_token.clone().unwrap_or_default();
        for (registry, username, password) in &logins {
            let url = format!("{}/api/docker/login", env.url);
            let body = serde_json::json!({
                "registry": registry,
                "username": username,
                "password": password,
            });
            match client.post(&url).header("X-Agent-Token", &token).json(&body).send().await {
                Ok(r) if r.status().is_success() => {
                    tracing::info!("Registry login '{}' propagated to agent '{}'", registry, env.name);
                }
                Ok(r) => {
                    tracing::warn!("Agent '{}' rejected login for '{}': HTTP {}", env.name, registry, r.status());
                }
                Err(e) => {
                    tracing::warn!("Failed to reach agent '{}' for login '{}': {}", env.name, registry, e);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dockpit=info,tower_http=info".into()),
        )
        .init();

    let db_path = std::env::var("DOCKPIT_DB_PATH")
        .unwrap_or_else(|_| "/data/dockpit.db".to_string());

    let db = db::Database::new(&db_path).expect("Failed to initialize database");
    let docker = docker::DockerClient::new().expect("Failed to connect to Docker");
    let stacks_mgr = stacks::StackManager::new();

    // Ensure Docker config dir is persistent via /data volume
    std::fs::create_dir_all("/data/.docker").ok();
    if !std::path::Path::new("/root/.docker").exists() {
        std::os::unix::fs::symlink("/data/.docker", "/root/.docker").ok();
    }

    // Seed default stack templates
    db.seed_default_templates();
    db.seed_default_group();

    // Restore Docker registry logins from DB
    restore_docker_logins(&db).await;

    let state = Arc::new(AppState {
        db, docker, stacks: stacks_mgr,
        update_check_running: std::sync::atomic::AtomicBool::new(false),
        vuln_scan_running: std::sync::atomic::AtomicBool::new(false),
        vuln_scan_total: std::sync::atomic::AtomicUsize::new(0),
        vuln_scan_done: std::sync::atomic::AtomicUsize::new(0),
        login_attempts: std::sync::Mutex::new(std::collections::HashMap::new()),
        ws_tokens: std::sync::Mutex::new(std::collections::HashMap::new()),
        env_cache: std::sync::RwLock::new(None),
    });

    // Start scheduler loop (checks for due jobs every 60 seconds)
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        // Initial event collection: fetch last 24 hours
        handlers::collect_events_since(scheduler_state.clone(), 86400).await;
        let mut tick = 0u64;
        loop {
            // Collect events every 30 seconds (last 60s window)
            handlers::collect_events_since(scheduler_state.clone(), 60).await;
            handlers::evaluate_alert_rules(scheduler_state.clone()).await;
            // Cleanup old audit log entries every hour (every 120th tick)
            if tick % 120 == 0 {
                scheduler_state.db.cleanup_old_audit();
                handlers::check_scheduled_backup(scheduler_state.clone()).await;
            }
            // Run scheduled jobs every 60 seconds (every 2nd tick)
            if tick % 2 == 0 {
                handlers::run_due_jobs(scheduler_state.clone()).await;
            }
            tick += 1;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    // Public routes
    let public_routes = Router::new()
        .route("/api/status", get(handlers::get_status))
        .route("/api/metrics", get(handlers::prometheus_metrics))
        .route("/api/setup", post(handlers::setup))
        .route("/api/login", post(handlers::login));

    // === VIEWER+ routes (any logged-in user) ===
    let viewer_routes = Router::new()
        .route("/api/home/servers", get(handlers::home_servers))
        .route("/api/dashboard", get(handlers::dashboard))
        .route("/api/profile", get(handlers::get_profile))
        .route("/api/profile/password", post(handlers::change_password))
        .route("/api/profile/totp/setup", post(handlers::totp_setup))
        .route("/api/profile/totp/verify", post(handlers::totp_verify))
        .route("/api/profile/totp/disable", post(handlers::totp_disable))
        .route("/api/environments", get(handlers::list_environments))
        .route("/api/environments/{id}/status", get(handlers::env_check_status))
        .route("/api/env/{env_id}/system", get(handlers::env_system_info))
        .route("/api/env/{env_id}/disk-usage", get(handlers::env_disk_usage))
        .route("/api/env/{env_id}/stats", get(handlers::env_stats))
        .route("/api/env/{env_id}/containers", get(handlers::env_containers))
        .route("/api/env/{env_id}/containers/{container_id}/logs", get(handlers::env_container_logs))
        .route("/api/env/{env_id}/health", get(handlers::env_health_checks))
        .route("/api/search/images", get(handlers::search_docker_hub))
        .route("/api/env/{env_id}/events", get(handlers::env_get_events))
        .route("/api/env/{env_id}/events/refresh", post(handlers::env_refresh_events))
        .route("/api/notifications", get(handlers::get_notifications))
        .route("/api/notifications/unread-count", get(handlers::get_unread_count))
        .route("/api/notifications/{id}/read", post(handlers::mark_notification_read))
        .route("/api/notifications/read-all", post(handlers::mark_all_notifications_read))
        .route("/api/notifications/{id}", delete(handlers::delete_notification))
        .route("/api/env/{env_id}/images", get(handlers::env_images))
        .route("/api/env/{env_id}/volumes", get(handlers::env_volumes))
        .route("/api/env/{env_id}/networks", get(handlers::env_networks))
        .route("/api/env/{env_id}/stacks", get(handlers::env_list_stacks))
        .route("/api/env/{env_id}/stacks/{name}", get(handlers::env_get_stack))
        .route("/api/updates/report", get(handlers::get_update_report))
        .route("/api/updates/status", get(handlers::get_update_check_status))
        .route("/api/ws-token", post(handlers::create_ws_token))
        .route("/api/refresh", post(handlers::refresh_token))
        .route("/api/dashboard-config", get(handlers::get_dashboard_config))
        .route("/api/dashboard-config", put(handlers::save_dashboard_config))
        .route("/api/env/{env_id}/containers/{container_id}/inspect", get(handlers::env_inspect_container))
        .route("/api/snapshots/{container_name}", get(handlers::get_container_snapshots))
        .route("/api/snippets/{container_name}", get(handlers::get_snippets))
        .route("/api/snapshots/diff/{id1}/{id2}", get(handlers::get_snapshot_diff))
        .route("/api/my-permissions", get(handlers::get_user_permissions_handler))
        .route("/api/profile/email", put(handlers::update_profile_email))
        .route("/api/settings", get(handlers::get_settings))
        .route("/api/settings", post(handlers::save_settings))
        .route("/api/settings/webhook/test", post(handlers::test_webhook))
        .route("/api/settings/email/test", post(handlers::test_email))
        .layer(middleware::from_fn(auth::auth_middleware));

    // === Action routes (permissions checked by frontend + group system) ===
    let action_routes = Router::new()
        .route("/api/env/{env_id}/containers/{container_id}/action", post(handlers::env_container_action))
        .route("/api/env/{env_id}/stacks/{name}/deploy", post(handlers::env_deploy_stack))
        .route("/api/env/{env_id}/stacks/{name}/stop", post(handlers::env_stop_stack))
        .route("/api/env/{env_id}/stacks/{name}/restart", post(handlers::env_restart_stack))
        .route("/api/environments", post(handlers::create_environment))
        .route("/api/environments/{id}", put(handlers::update_environment))
        .route("/api/environments/{id}", delete(handlers::delete_environment))
        .route("/api/environments/{id}/pause", put(handlers::toggle_env_paused))
        .route("/api/agents/discover", post(handlers::discover_agents))
        .route("/api/registries", get(handlers::list_registries))
        .route("/api/registries", post(handlers::add_registry))
        .route("/api/registries/{registry}", delete(handlers::remove_registry))
        .route("/api/env/{env_id}/containers/{container_id}/check-update", post(handlers::env_check_container_update))
        .route("/api/env/{env_id}/containers/{container_id}/recreate", post(handlers::env_recreate_container))
        .route("/api/env/{env_id}/containers/{container_id}/migrate", post(handlers::env_migrate_container))
        .route("/api/env/{env_id}/containers/{container_id}/rollback", post(handlers::rollback_container))
        .route("/api/snapshots/delete/{id}", delete(handlers::delete_snapshot))
        .route("/api/snippets/create/{container_name}", post(handlers::create_snippet))
        .route("/api/snippets/item/{id}", delete(handlers::delete_snippet_handler))
        .route("/api/alert-rules", get(handlers::get_alert_rules))
        .route("/api/alert-rules", post(handlers::create_alert_rule))
        .route("/api/alert-rules/{id}", put(handlers::toggle_alert_rule))
        .route("/api/alert-rules/{id}", delete(handlers::delete_alert_rule))
        .route("/api/env/{env_id}/stacks/{name}/migrate", post(handlers::env_migrate_stack))
        .route("/api/env/{env_id}/images/pull", post(handlers::env_pull_image))
        .route("/api/env/{env_id}/images/prune", post(handlers::env_prune_images))
        .route("/api/env/{env_id}/images/{image_id}", delete(handlers::env_remove_image))
        .route("/api/env/{env_id}/volumes/prune", post(handlers::env_prune_volumes))
        .route("/api/env/{env_id}/volumes/{name}", delete(handlers::env_remove_volume))
        .route("/api/env/{env_id}/networks/prune", post(handlers::env_prune_networks))
        .route("/api/env/{env_id}/networks/{network_id}", delete(handlers::env_remove_network))
        .route("/api/env/{env_id}/stacks", post(handlers::env_create_stack))
        .route("/api/env/{env_id}/stacks/{name}", put(handlers::env_update_stack))
        .route("/api/env/{env_id}/stacks/{name}", delete(handlers::env_delete_stack))
        .route("/api/env/{env_id}/stacks/{name}/redeploy", post(handlers::env_redeploy_stack))
        .route("/api/updates/check", post(handlers::run_update_check))
        .route("/api/updates/report", delete(handlers::clear_update_report))
        .route("/api/audit", get(handlers::get_audit_log))
        .route("/api/backups", get(handlers::list_backups))
        .route("/api/backups", post(handlers::create_backup))
        .route("/api/backups/upload-restore", post(handlers::upload_restore))
        .route("/api/backups/restore/{filename}", post(handlers::restore_backup))
        .route("/api/backups/{filename}", get(handlers::download_backup))
        .route("/api/backups/{filename}", delete(handlers::delete_backup))
        .route("/api/templates", get(handlers::list_templates))
        .route("/api/templates", post(handlers::create_template))
        .route("/api/templates/{id}", get(handlers::get_template))
        .route("/api/templates/{id}", put(handlers::update_template))
        .route("/api/templates/{id}", delete(handlers::delete_template))
        .route("/api/env/{env_id}/vulnerabilities", get(handlers::env_get_vulnerabilities))
        .route("/api/env/{env_id}/vulnerabilities/scan", post(handlers::env_scan_vulnerabilities))
        .route("/api/env/{env_id}/vulnerabilities/status", get(handlers::env_vuln_scan_status))
        .route("/api/env/{env_id}/vulnerabilities/history/{image}", get(handlers::env_get_scan_history))
        .route("/api/env/{env_id}/vulnerabilities/scan/{image}", post(handlers::env_scan_single_image))
        .route("/api/scheduled-jobs", get(handlers::list_scheduled_jobs))
        .route("/api/scheduled-jobs", post(handlers::create_scheduled_job))
        .route("/api/scheduled-jobs/{id}", put(handlers::update_scheduled_job))
        .route("/api/scheduled-jobs/{id}", delete(handlers::delete_scheduled_job))
        .route("/api/scheduled-jobs/{id}/run", post(handlers::run_scheduled_job))
        .layer(middleware::from_fn(auth::auth_middleware));

    // === User management routes (permission checked in handlers) ===
    let super_admin_routes = Router::new()
        .route("/api/users", get(handlers::list_users))
        .route("/api/users", post(handlers::create_user))
        .route("/api/users/{id}", put(handlers::update_user))
        .route("/api/users/{id}", delete(handlers::delete_user))
        .route("/api/users/{id}/reset-mfa", post(handlers::reset_user_mfa))
        .route("/api/groups", get(handlers::list_groups))
        .route("/api/groups", post(handlers::create_group))
        .route("/api/groups/{id}", put(handlers::update_group))
        .route("/api/groups/{id}", delete(handlers::delete_group))
        .route("/api/users/{id}/groups", put(handlers::set_user_groups_handler))
        .layer(middleware::from_fn(auth::auth_middleware));

    // WebSocket routes (auth via query param)
    let ws_routes = Router::new()
        .route("/api/env/{env_id}/containers/{container_id}/terminal", get(handlers::env_container_terminal))
        .route("/api/env/{env_id}/stats/live", get(handlers::env_stats_live))
        .route("/api/env/{env_id}/host-terminal", get(handlers::host_terminal));

    let app = Router::new()
        .merge(public_routes)
        .merge(viewer_routes)
        .merge(action_routes)
        .merge(super_admin_routes)
        .merge(ws_routes)
        .fallback(serve_frontend)
        .layer(middleware::from_fn(auth::csrf_middleware))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let http_port = std::env::var("DOCKPIT_PORT")
        .unwrap_or_else(|_| "5533".to_string())
        .parse::<u16>()
        .unwrap_or(5533);

    let https_port = std::env::var("DOCKPIT_HTTPS_PORT")
        .unwrap_or_else(|_| "5539".to_string())
        .parse::<u16>()
        .unwrap_or(5539);

    let cert_dir = std::env::var("DOCKPIT_CERT_DIR")
        .unwrap_or_else(|_| "/data/certs".to_string());

    // Start HTTP
    let http_app = app.clone();
    let http_handle = tokio::spawn(async move {
        let addr = format!("0.0.0.0:{}", http_port);
        tracing::info!("HTTP  on {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, http_app).await.unwrap();
    });

    // Start HTTPS with self-signed cert
    let https_handle = tokio::spawn(async move {
        let cert_path = format!("{}/cert.pem", cert_dir);
        let key_path = format!("{}/key.pem", cert_dir);

        // Generate self-signed cert if not exists
        if !std::path::Path::new(&cert_path).exists() {
            tracing::info!("Generating self-signed TLS certificate...");
            std::fs::create_dir_all(&cert_dir).ok();

            let subject_alt_names = vec!["localhost".to_string(), "dockpit".to_string()];
            let cert = rcgen::generate_simple_self_signed(subject_alt_names)
                .expect("Failed to generate certificate");

            std::fs::write(&cert_path, cert.cert.pem()).expect("Failed to write cert");
            std::fs::write(&key_path, cert.key_pair.serialize_pem()).expect("Failed to write key");
            tracing::info!("Certificate generated at {}", cert_dir);
        }

        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert_path, &key_path)
            .await
            .expect("Failed to load TLS config");

        let addr = format!("0.0.0.0:{}", https_port);
        tracing::info!("HTTPS on {}", addr);

        axum_server::bind_rustls(addr.parse().unwrap(), tls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    tokio::select! {
        _ = http_handle => {},
        _ = https_handle => {},
    }
}
