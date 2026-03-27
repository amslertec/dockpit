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
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref().to_string())],
            file.data.to_vec(),
        )
            .into_response()
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
        Some(file) => Html(String::from_utf8_lossy(&file.data).to_string()).into_response(),
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

    // Restore Docker registry logins from DB
    restore_docker_logins(&db).await;

    let state = Arc::new(AppState {
        db, docker, stacks: stacks_mgr,
        update_check_running: std::sync::atomic::AtomicBool::new(false),
    });

    // Start scheduler loop (checks for due jobs every 60 seconds)
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        loop {
            handlers::run_due_jobs(scheduler_state.clone()).await;
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    // Public routes
    let public_routes = Router::new()
        .route("/api/status", get(handlers::get_status))
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
        .route("/api/env/{env_id}/images", get(handlers::env_images))
        .route("/api/env/{env_id}/volumes", get(handlers::env_volumes))
        .route("/api/env/{env_id}/networks", get(handlers::env_networks))
        .route("/api/env/{env_id}/stacks", get(handlers::env_list_stacks))
        .route("/api/env/{env_id}/stacks/{name}", get(handlers::env_get_stack))
        .route("/api/updates/report", get(handlers::get_update_report))
        .route("/api/updates/status", get(handlers::get_update_check_status))
        .layer(middleware::from_fn(auth::auth_middleware));

    // === EDITOR+ routes (start/stop/restart containers, deploy stacks) ===
    let editor_routes = Router::new()
        .route("/api/env/{env_id}/containers/{container_id}/action", post(handlers::env_container_action))
        .route("/api/env/{env_id}/stacks/{name}/deploy", post(handlers::env_deploy_stack))
        .route("/api/env/{env_id}/stacks/{name}/stop", post(handlers::env_stop_stack))
        .route("/api/env/{env_id}/stacks/{name}/restart", post(handlers::env_restart_stack))
        .layer(middleware::from_fn(auth::editor_middleware));

    // === ADMIN+ routes (create/delete/modify resources) ===
    let admin_routes = Router::new()
        .route("/api/environments", post(handlers::create_environment))
        .route("/api/environments/{id}", put(handlers::update_environment))
        .route("/api/environments/{id}", delete(handlers::delete_environment))
        .route("/api/registries", get(handlers::list_registries))
        .route("/api/registries", post(handlers::add_registry))
        .route("/api/registries/{registry}", delete(handlers::remove_registry))
        .route("/api/env/{env_id}/containers/{container_id}/check-update", post(handlers::env_check_container_update))
        .route("/api/env/{env_id}/containers/{container_id}/recreate", post(handlers::env_recreate_container))
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
        .route("/api/settings", get(handlers::get_settings))
        .route("/api/settings", post(handlers::save_settings))
        .route("/api/settings/webhook/test", post(handlers::test_webhook))
        .route("/api/updates/check", post(handlers::run_update_check))
        .route("/api/updates/report", delete(handlers::clear_update_report))
        .route("/api/scheduled-jobs", get(handlers::list_scheduled_jobs))
        .route("/api/scheduled-jobs", post(handlers::create_scheduled_job))
        .route("/api/scheduled-jobs/{id}", put(handlers::update_scheduled_job))
        .route("/api/scheduled-jobs/{id}", delete(handlers::delete_scheduled_job))
        .route("/api/scheduled-jobs/{id}/run", post(handlers::run_scheduled_job))
        .layer(middleware::from_fn(auth::admin_middleware));

    // === SUPER_ADMIN only routes (user management) ===
    let super_admin_routes = Router::new()
        .route("/api/users", get(handlers::list_users))
        .route("/api/users", post(handlers::create_user))
        .route("/api/users/{id}", put(handlers::update_user))
        .route("/api/users/{id}", delete(handlers::delete_user))
        .route("/api/users/{id}/reset-mfa", post(handlers::reset_user_mfa))
        .layer(middleware::from_fn(auth::super_admin_middleware));

    // WebSocket routes (auth via query param)
    let ws_routes = Router::new()
        .route("/api/env/{env_id}/containers/{container_id}/terminal", get(handlers::env_container_terminal))
        .route("/api/env/{env_id}/stats/live", get(handlers::env_stats_live));

    let app = Router::new()
        .merge(public_routes)
        .merge(viewer_routes)
        .merge(editor_routes)
        .merge(admin_routes)
        .merge(super_admin_routes)
        .merge(ws_routes)
        .fallback(serve_frontend)
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
