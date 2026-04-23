use axum::{
    extract::{Path, Query, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

use crate::auth;
use crate::db::Database;
use crate::docker::DockerClient;
use crate::models::*;
use crate::stacks::StackManager;

pub struct AppState {
    pub db: Database,
    pub docker: DockerClient,
    pub stacks: StackManager,
    pub update_check_running: std::sync::atomic::AtomicBool,
    pub vuln_scan_running: std::sync::atomic::AtomicBool,
    pub vuln_scan_total: std::sync::atomic::AtomicUsize,
    pub vuln_scan_done: std::sync::atomic::AtomicUsize,
    pub login_attempts: std::sync::Mutex<std::collections::HashMap<String, (u32, std::time::Instant)>>,
    pub ws_tokens: std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>,
    pub env_cache: std::sync::RwLock<Option<Vec<EnvironmentInfo>>>,
}

// === Agent Proxy Helpers ===

async fn agent_get<T: DeserializeOwned + Serialize>(
    env: &EnvironmentInfo,
    path: &str,
) -> Json<ApiResponse<T>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let url = format!("{}{}", env.url, path);
    match client
        .get(&url)
        .header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<ApiResponse<T>>().await {
            Ok(data) => Json(data),
            Err(e) => Json(ApiResponse::err(format!("Antwort ungültig: {}", e))),
        },
        Err(e) => Json(ApiResponse::err(format!("Agent nicht erreichbar: {}", e))),
    }
}

async fn agent_post<B: Serialize, T: DeserializeOwned + Serialize>(
    env: &EnvironmentInfo,
    path: &str,
    body: &B,
) -> Json<ApiResponse<T>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let url = format!("{}{}", env.url, path);
    match client
        .post(&url)
        .header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
        .json(body)
        .send()
        .await
    {
        Ok(resp) => match resp.json::<ApiResponse<T>>().await {
            Ok(data) => Json(data),
            Err(e) => Json(ApiResponse::err(format!("Antwort ungültig: {}", e))),
        },
        Err(e) => Json(ApiResponse::err(format!("Agent nicht erreichbar: {}", e))),
    }
}

async fn agent_put<B: Serialize, T: DeserializeOwned + Serialize>(
    env: &EnvironmentInfo,
    path: &str,
    body: &B,
) -> Json<ApiResponse<T>> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
    let url = format!("{}{}", env.url, path);
    match client.put(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).json(body).send().await {
        Ok(resp) => match resp.json::<ApiResponse<T>>().await { Ok(d) => Json(d), Err(e) => Json(ApiResponse::err(e.to_string())) },
        Err(e) => Json(ApiResponse::err(format!("Agent nicht erreichbar: {}", e))),
    }
}

async fn agent_del<T: DeserializeOwned + Serialize>(
    env: &EnvironmentInfo,
    path: &str,
) -> Json<ApiResponse<T>> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
    let url = format!("{}{}", env.url, path);
    match client.delete(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
        Ok(resp) => match resp.json::<ApiResponse<T>>().await { Ok(d) => Json(d), Err(e) => Json(ApiResponse::err(e.to_string())) },
        Err(e) => Json(ApiResponse::err(format!("Agent nicht erreichbar: {}", e))),
    }
}

fn get_env(state: &AppState, env_id: &str) -> Result<EnvironmentInfo, Json<ApiResponse<String>>> {
    state.db.get_environment(env_id).ok_or_else(|| Json(ApiResponse::err("Umgebung nicht gefunden")))
}

// === Prometheus Metrics ===

pub async fn prometheus_metrics(
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let mut lines = Vec::new();

    // Environments
    let envs = state.db.get_environments();
    let online = envs.iter().filter(|e| e.is_local || e.status == "online").count();
    let offline = envs.len() - online;
    lines.push("# HELP dockpit_environments_total Total environments by status".into());
    lines.push("# TYPE dockpit_environments_total gauge".into());
    lines.push(format!("dockpit_environments_total{{status=\"online\"}} {}", online));
    lines.push(format!("dockpit_environments_total{{status=\"offline\"}} {}", offline));

    // Per-environment stats
    lines.push("# HELP dockpit_containers_total Containers by environment and state".into());
    lines.push("# TYPE dockpit_containers_total gauge".into());
    lines.push("# HELP dockpit_images_total Images by environment".into());
    lines.push("# TYPE dockpit_images_total gauge".into());
    lines.push("# HELP dockpit_volumes_total Volumes by environment".into());
    lines.push("# TYPE dockpit_volumes_total gauge".into());
    lines.push("# HELP dockpit_networks_total Networks by environment".into());
    lines.push("# TYPE dockpit_networks_total gauge".into());

    // Collect per-env stats in parallel
    let mut remote_handles = Vec::new();
    let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();
    for env in &envs {
        let ename = env.name.replace('"', "");
        if env.is_local {
            let stats = state.docker.get_dashboard_stats().await;
            lines.push(format!("dockpit_containers_total{{env=\"{}\",state=\"running\"}} {}", ename, stats.containers_running));
            lines.push(format!("dockpit_containers_total{{env=\"{}\",state=\"stopped\"}} {}", ename, stats.containers_stopped));
            lines.push(format!("dockpit_images_total{{env=\"{}\"}} {}", ename, stats.images_total));
            lines.push(format!("dockpit_volumes_total{{env=\"{}\"}} {}", ename, stats.volumes_total));
            lines.push(format!("dockpit_networks_total{{env=\"{}\"}} {}", ename, stats.networks_total));
        } else {
            let url = format!("{}/api/system", env.url);
            let token = env.agent_token.clone().unwrap_or_default();
            let c = client.clone();
            let name = ename.clone();
            remote_handles.push(tokio::spawn(async move {
                let mut env_lines = Vec::new();
                if let Ok(resp) = c.get(&url).header("X-Agent-Token", &token).send().await {
                    if let Ok(data) = resp.json::<ApiResponse<SystemInfo>>().await {
                        if let Some(info) = data.data {
                            env_lines.push(format!("dockpit_containers_total{{env=\"{}\",state=\"running\"}} {}", name, info.containers_running));
                            env_lines.push(format!("dockpit_containers_total{{env=\"{}\",state=\"stopped\"}} {}", name, info.containers_stopped));
                            env_lines.push(format!("dockpit_images_total{{env=\"{}\"}} {}", name, info.images));
                            env_lines.push(format!("dockpit_volumes_total{{env=\"{}\"}} {}", name, info.volumes));
                            env_lines.push(format!("dockpit_networks_total{{env=\"{}\"}} {}", name, info.networks));
                        }
                    }
                }
                env_lines
            }));
        }
    }
    for handle in remote_handles {
        if let Ok(env_lines) = handle.await {
            lines.extend(env_lines);
        }
    }

    // Stacks (local only for speed)
    if let Ok(stacks) = state.stacks.list_stacks().await {
        let running = stacks.iter().filter(|s| s.status == "running").count();
        let stopped = stacks.iter().filter(|s| s.status == "stopped").count();
        let partial = stacks.iter().filter(|s| s.status == "partial").count();
        lines.push("# HELP dockpit_stacks_total Stacks by status".into());
        lines.push("# TYPE dockpit_stacks_total gauge".into());
        lines.push(format!("dockpit_stacks_total{{status=\"running\"}} {}", running));
        lines.push(format!("dockpit_stacks_total{{status=\"stopped\"}} {}", stopped));
        lines.push(format!("dockpit_stacks_total{{status=\"partial\"}} {}", partial));
    }

    // Updates
    let updates = state.db.get_latest_update_checks();
    let outdated = updates.iter().filter(|u| u.outdated).count();
    lines.push("# HELP dockpit_updates_outdated Containers with outdated images".into());
    lines.push("# TYPE dockpit_updates_outdated gauge".into());
    lines.push(format!("dockpit_updates_outdated {}", outdated));

    // Users
    let users = state.db.list_users().len();
    lines.push("# HELP dockpit_users_total Total users".into());
    lines.push("# TYPE dockpit_users_total gauge".into());
    lines.push(format!("dockpit_users_total {}", users));

    // Notifications
    let unread = state.db.get_unread_count();
    lines.push("# HELP dockpit_notifications_unread Unread notifications".into());
    lines.push("# TYPE dockpit_notifications_unread gauge".into());
    lines.push(format!("dockpit_notifications_unread {}", unread));

    // Scheduled jobs
    let jobs = state.db.get_scheduled_jobs(None);
    let enabled = jobs.iter().filter(|j| j.enabled).count();
    let disabled = jobs.len() - enabled;
    lines.push("# HELP dockpit_scheduled_jobs_total Scheduled jobs by status".into());
    lines.push("# TYPE dockpit_scheduled_jobs_total gauge".into());
    lines.push(format!("dockpit_scheduled_jobs_total{{status=\"enabled\"}} {}", enabled));
    lines.push(format!("dockpit_scheduled_jobs_total{{status=\"disabled\"}} {}", disabled));

    lines.push(String::new()); // trailing newline

    let body = lines.join("\n");
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    ).into_response()
}

// === Setup & Auth ===

pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<ApiResponse<AppStatus>> {
    Json(ApiResponse::ok(AppStatus {
        setup_complete: state.db.is_setup_complete(),
    }))
}

pub async fn setup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    if state.db.is_setup_complete() {
        return Ok(Json(ApiResponse::err("Setup bereits abgeschlossen")));
    }

    if req.username.len() < 3 || req.password.len() < 6 {
        return Ok(Json(ApiResponse::err(
            "Benutzername min. 3 Zeichen, Passwort min. 6 Zeichen",
        )));
    }

    let password_hash = auth::hash_password(&req.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let user_id = uuid::Uuid::new_v4().to_string();

    state.db.create_user_with_role(&user_id, &req.username, &password_hash, "super_admin")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state.db.ensure_local_environment();

    let token = auth::create_token(&user_id, &req.username, "super_admin")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::ok(LoginResponse {
        token,
        username: req.username,
    })))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest2FA>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    // Rate limiting: max 5 attempts per 15 minutes per username
    {
        let mut attempts = state.login_attempts.lock().unwrap();
        let key = req.username.to_lowercase();
        if let Some((count, since)) = attempts.get(&key) {
            if since.elapsed() < Duration::from_secs(900) && *count >= 5 {
                state.db.log_audit(&req.username, "login_blocked", None, Some("Too many attempts"));
                return Ok(Json(ApiResponse::err("Too many login attempts. Try again in 15 minutes.")));
            }
            if since.elapsed() >= Duration::from_secs(900) {
                attempts.remove(&key);
            }
        }
    }

    match state.db.get_user_by_username(&req.username) {
        Some((id, username, hash)) if auth::verify_password(&req.password, &hash) => {
            // Check TOTP if enabled
            if let Some(secret) = state.db.get_totp_secret(&username) {
                let code = match &req.totp_code {
                    Some(c) if !c.is_empty() => c,
                    _ => return Ok(Json(ApiResponse::err("2FA-Code erforderlich"))),
                };
                if !auth::verify_totp(&secret, code) {
                    // Try backup codes
                    let mut used_backup = false;
                    if let Some(codes_json) = state.db.get_backup_codes(&username) {
                        if let Ok(mut codes) = serde_json::from_str::<Vec<String>>(&codes_json) {
                            if let Some(pos) = codes.iter().position(|c| c == code) {
                                codes.remove(pos);
                                let updated = serde_json::to_string(&codes).unwrap_or_default();
                                state.db.set_backup_codes(&username, Some(&updated)).ok();
                                used_backup = true;
                            }
                        }
                    }
                    if !used_backup {
                        return Ok(Json(ApiResponse::err("Ungültiger 2FA-Code")));
                    }
                }
            }
            let role = state.db.get_user_role(&username).unwrap_or_else(|| "admin".to_string());
            let token = auth::create_token(&id, &username, &role)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            // Clear rate limit on success
            { let mut attempts = state.login_attempts.lock().unwrap(); attempts.remove(&req.username.to_lowercase()); }
            state.db.log_audit(&username, "login", None, Some("Login successful"));
            Ok(Json(ApiResponse::ok(LoginResponse { token, username })))
        }
        _ => {
            // Increment failed attempt counter
            {
                let mut attempts = state.login_attempts.lock().unwrap();
                let key = req.username.to_lowercase();
                let entry = attempts.entry(key).or_insert((0, std::time::Instant::now()));
                entry.0 += 1;
            }
            state.db.log_audit(&req.username, "login_failed", None, Some("Invalid credentials"));
            Ok(Json(ApiResponse::err("Ungültige Anmeldedaten")))
        }
    }
}

pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<LoginResponse>> {
    // Grace window: accept tokens expired up to 7 days ago. Signature is still verified.
    const REFRESH_GRACE_SECONDS: i64 = 7 * 24 * 60 * 60;

    let header = match headers.get("Authorization").and_then(|v| v.to_str().ok()) {
        Some(h) => h,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    let token = match header.strip_prefix("Bearer ") {
        Some(t) => t,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    let claims = match auth::validate_token_allow_expired(token, REFRESH_GRACE_SECONDS) {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    // Verify user still exists and get current role
    let role = match state.db.get_user_role(&claims.username) {
        Some(r) => r,
        None => return Json(ApiResponse::err("Benutzer nicht gefunden")),
    };
    match auth::create_token(&claims.sub, &claims.username, &role) {
        Ok(token) => Json(ApiResponse::ok(LoginResponse { token, username: claims.username })),
        Err(_) => Json(ApiResponse::err("Token-Erneuerung fehlgeschlagen")),
    }
}

// === Profile ===

pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<UserProfile>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    let role = state.db.get_user_role(&claims.username)
        .unwrap_or_else(|| "admin".to_string());

    let totp_enabled = state.db.get_totp_secret(&claims.username).is_some();

    let email = state.db.get_user_email(&claims.username);
    let email_notifications = state.db.get_user_email_notifications(&claims.username);

    Json(ApiResponse::ok(UserProfile {
        username: claims.username,
        role,
        totp_enabled,
        email,
        email_notifications,
    }))
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    if req.new_password.len() < 6 {
        return Json(ApiResponse::err("Neues Passwort muss min. 6 Zeichen haben"));
    }

    let user = match state.db.get_user_by_username(&claims.username) {
        Some(u) => u,
        None => return Json(ApiResponse::err("Benutzer nicht gefunden")),
    };

    if !auth::verify_password(&req.current_password, &user.2) {
        return Json(ApiResponse::err("Aktuelles Passwort ist falsch"));
    }

    let new_hash = match auth::hash_password(&req.new_password) {
        Ok(h) => h,
        Err(_) => return Json(ApiResponse::err("Fehler beim Hashen")),
    };

    match state.db.update_password(&claims.username, &new_hash) {
        Ok(_) => Json(ApiResponse::ok("Passwort geändert".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn totp_setup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<TotpSetupResponse>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    // Generate new TOTP secret
    let (secret, otpauth_url, qr_base64) = auth::generate_totp_setup(&claims.username);

    // Store secret temporarily - will be confirmed with verify
    state.db.set_totp_secret(&claims.username, Some(&secret)).ok();

    Json(ApiResponse::ok(TotpSetupResponse {
        secret,
        qr_code: qr_base64,
        otpauth_url,
        backup_codes: None,
    }))
}

pub async fn totp_verify(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TotpVerifyRequest>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    let secret = match state.db.get_totp_secret(&claims.username) {
        Some(s) => s,
        None => return Json(ApiResponse::err("2FA nicht eingerichtet")),
    };

    if auth::verify_totp(&secret, &req.code) {
        let codes = auth::generate_backup_codes();
        let codes_json = serde_json::to_string(&codes).unwrap_or_default();
        state.db.set_backup_codes(&claims.username, Some(&codes_json)).ok();
        Json(ApiResponse::ok(serde_json::to_string(&codes).unwrap_or_else(|_| "2FA aktiviert".to_string())))
    } else {
        // Wrong code - remove the secret again
        state.db.set_totp_secret(&claims.username, None).ok();
        Json(ApiResponse::err("Ungültiger Code. 2FA wurde nicht aktiviert."))
    }
}

pub async fn totp_disable(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TotpVerifyRequest>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    let secret = match state.db.get_totp_secret(&claims.username) {
        Some(s) => s,
        None => return Json(ApiResponse::err("2FA ist nicht aktiviert")),
    };

    if auth::verify_totp(&secret, &req.code) {
        state.db.set_totp_secret(&claims.username, None).ok();
        state.db.set_backup_codes(&claims.username, None).ok();
        Json(ApiResponse::ok("2FA deaktiviert".to_string()))
    } else {
        Json(ApiResponse::err("Ungültiger Code"))
    }
}

// === Dashboard Config ===

pub async fn get_dashboard_config(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    match state.db.get_dashboard_config(&claims.username) {
        Some(json) => Json(ApiResponse::ok(json)),
        None => Json(ApiResponse::ok("".to_string())),
    }
}

pub async fn save_dashboard_config(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: String,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    match state.db.save_dashboard_config(&claims.username, &body) {
        Ok(_) => Json(ApiResponse::ok("OK".to_string())),
        Err(_) => Json(ApiResponse::err("Fehler beim Speichern")),
    }
}

fn extract_claims(headers: &axum::http::HeaderMap) -> Option<Claims> {
    let header = headers.get("Authorization")?.to_str().ok()?;
    let token = header.strip_prefix("Bearer ")?;
    auth::validate_token(token).ok()
}

fn has_permission(state: &AppState, claims: &Claims, perm: &str) -> bool {
    if claims.role == "super_admin" { return true; }
    let perms = state.db.get_user_permissions(&claims.sub);
    perms.iter().any(|p| p == perm)
}

#[allow(dead_code)]
fn send_email_sync(state: &AppState, subject: &str, body: &str) {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};

    let host = match state.db.get_setting("smtp_host") {
        Some(h) if !h.is_empty() => h,
        _ => return,
    };
    let port: u16 = state.db.get_setting("smtp_port").and_then(|p| p.parse().ok()).unwrap_or(587);
    let user = state.db.get_setting("smtp_user").unwrap_or_default();
    let pass = state.db.get_setting("smtp_pass").unwrap_or_default();
    let from_addr = match state.db.get_setting("smtp_from") {
        Some(f) if !f.is_empty() => f,
        _ => return,
    };
    let use_tls = state.db.get_setting("smtp_tls").map(|v| v == "true").unwrap_or(true);

    let recipients = state.db.get_users_for_email_notification("");
    if recipients.is_empty() { return; }

    let creds = Credentials::new(user, pass);
    let mailer = if use_tls {
        match SmtpTransport::starttls_relay(&host) {
            Ok(b) => b.port(port).credentials(creds).build(),
            Err(_) => return,
        }
    } else {
        SmtpTransport::builder_dangerous(&host).port(port).credentials(creds).build()
    };

    for (_username, email) in &recipients {
        let to_addr = match email.parse() { Ok(a) => a, Err(_) => continue };
        let from = match from_addr.parse() { Ok(a) => a, Err(_) => continue };
        let msg = match Message::builder()
            .from(from)
            .to(to_addr)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(format!("{}\n\n-- DockPit", body)) {
                Ok(m) => m,
                Err(_) => continue,
            };
        if let Err(e) = mailer.send(&msg) {
            tracing::warn!("Email to {} failed: {}", email, e);
        }
    }
}

#[allow(dead_code)]
fn notify_and_email(state: &AppState, ntype: &str, title: &str, message: &str) {
    state.db.create_notification(ntype, title, message).ok();
    let subject = format!("DockPit: {}", title);
    send_email_sync(state, &subject, message);
}

pub async fn test_email(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<String>> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};

    let host = match state.db.get_setting("smtp_host") {
        Some(h) if !h.is_empty() => h,
        _ => return Json(ApiResponse::err("SMTP Host nicht konfiguriert")),
    };
    let port: u16 = state.db.get_setting("smtp_port").and_then(|p| p.parse().ok()).unwrap_or(587);
    let user = state.db.get_setting("smtp_user").unwrap_or_default();
    let pass = state.db.get_setting("smtp_pass").unwrap_or_default();
    let from_addr = match state.db.get_setting("smtp_from") {
        Some(f) if !f.is_empty() => f,
        _ => return Json(ApiResponse::err("Absender nicht konfiguriert")),
    };
    let use_tls = state.db.get_setting("smtp_tls").map(|v| v == "true").unwrap_or(true);

    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    let to_email = match state.db.get_user_email(&claims.username) {
        Some(e) if !e.is_empty() => e,
        _ => return Json(ApiResponse::err("Keine Email für deinen Account hinterlegt")),
    };

    let creds = Credentials::new(user, pass);
    let mailer = if use_tls {
        match SmtpTransport::starttls_relay(&host) {
            Ok(b) => b.port(port).credentials(creds).build(),
            Err(e) => return Json(ApiResponse::err(format!("SMTP: {}", e))),
        }
    } else {
        SmtpTransport::builder_dangerous(&host).port(port).credentials(creds).build()
    };

    let from = match from_addr.parse() {
        Ok(a) => a,
        Err(_) => return Json(ApiResponse::err("Ungültige Absender-Adresse")),
    };
    let to = match to_email.parse() {
        Ok(a) => a,
        Err(_) => return Json(ApiResponse::err("Ungültige Empfänger-Adresse")),
    };

    let msg = match Message::builder()
        .from(from)
        .to(to)
        .subject("DockPit Test Email")
        .header(ContentType::TEXT_PLAIN)
        .body("This is a test email from DockPit.\n\nIf you received this email, your email configuration is working correctly.\n\n-- DockPit".to_string()) {
            Ok(m) => m,
            Err(e) => return Json(ApiResponse::err(format!("Build: {}", e))),
        };

    match mailer.send(&msg) {
        Ok(_) => Json(ApiResponse::ok(format!("Test-Email an {} gesendet", to_email))),
        Err(e) => Json(ApiResponse::err(format!("Versand: {}", e))),
    }
}

pub async fn update_profile_email(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    let email = req.get("email").and_then(|v| v.as_str()).unwrap_or("");
    state.db.update_user_email(&claims.username, if email.is_empty() { None } else { Some(email) }).ok();

    if let Some(notif) = req.get("email_notifications").and_then(|v| v.as_bool()) {
        state.db.set_email_notifications(&claims.username, notif).ok();
    }

    Json(ApiResponse::ok("Email aktualisiert".to_string()))
}

fn audit_user(headers: &axum::http::HeaderMap) -> String {
    headers.get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|token| crate::auth::validate_token(token).ok())
        .map(|claims| claims.username)
        .unwrap_or_else(|| "system".to_string())
}

fn env_name(state: &AppState, env_id: &str) -> String {
    state.db.get_environment(env_id).map(|e| e.name).unwrap_or_else(|| env_id.to_string())
}

// === Groups ===

pub async fn list_groups(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let groups = state.db.list_groups();
    let result: Vec<serde_json::Value> = groups.iter().map(|(id, name, desc, is_default, created_at, color)| {
        let members = state.db.get_group_members(*id);
        let permissions = state.db.get_group_permissions(*id);
        serde_json::json!({
            "id": id, "name": name, "description": desc,
            "is_default": is_default, "created_at": created_at,
            "color": color,
            "member_count": members.len(),
            "members": members.iter().map(|(uid, uname)| serde_json::json!({"id": uid, "username": uname})).collect::<Vec<_>>(),
            "permissions": permissions,
        })
    }).collect();
    Json(ApiResponse::ok(result))
}

pub async fn create_group(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let description = req.get("description").and_then(|v| v.as_str());
    if name.is_empty() { return Json(ApiResponse::err("Gruppenname erforderlich")); }
    let color = req.get("color").and_then(|v| v.as_str());
    match state.db.create_group(name, description, color) {
        Ok(id) => {
            // Set permissions if provided
            if let Some(perms) = req.get("permissions").and_then(|v| v.as_array()) {
                let perm_strs: Vec<String> = perms.iter().filter_map(|p| p.as_str().map(String::from)).collect();
                state.db.set_group_permissions(id, &perm_strs);
            }
            state.db.log_audit(&audit_user(&headers), "group_create", Some(name), None);
            Json(ApiResponse::ok("Gruppe erstellt".to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn update_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    // Update name/description/color if provided
    if let Some(name) = req.get("name").and_then(|v| v.as_str()) {
        let desc = req.get("description").and_then(|v| v.as_str());
        let color = req.get("color").and_then(|v| v.as_str());
        state.db.update_group_info(id, name, desc, color).ok();
    }
    // Update permissions if provided
    if let Some(perms) = req.get("permissions").and_then(|v| v.as_array()) {
        let perm_strs: Vec<String> = perms.iter().filter_map(|p| p.as_str().map(String::from)).collect();
        state.db.set_group_permissions(id, &perm_strs);
    }
    Json(ApiResponse::ok("Aktualisiert".to_string()))
}

pub async fn delete_group(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Json<ApiResponse<String>> {
    match state.db.delete_group(id) {
        Ok(_) => {
            state.db.log_audit(&audit_user(&headers), "group_delete", None, None);
            Json(ApiResponse::ok("Gruppe gelöscht".to_string()))
        }
        Err(_) => Json(ApiResponse::err("Standardgruppe kann nicht gelöscht werden")),
    }
}

pub async fn set_user_groups_handler(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let group_ids: Vec<i64> = req.get("group_ids").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();
    state.db.set_user_groups(&user_id, &group_ids);
    Json(ApiResponse::ok("Gruppen aktualisiert".to_string()))
}

pub async fn get_user_permissions_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<serde_json::Value>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    let role = state.db.get_user_role(&claims.username).unwrap_or_else(|| "viewer".to_string());
    // super_admin always gets all permissions
    if role == "super_admin" {
        return Json(ApiResponse::ok(serde_json::json!({
            "role": role,
            "permissions": ["*"],
        })));
    }
    // All other roles: permissions come from group memberships
    let permissions = state.db.get_user_permissions(&claims.sub);
    Json(ApiResponse::ok(serde_json::json!({
        "role": role,
        "permissions": permissions,
    })))
}

// === User Management (super_admin only) ===

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    if !has_permission(&state, &claims, "action.user_management") {
        return Json(ApiResponse::err("Keine Berechtigung"));
    }

    let users = state.db.list_users()
        .into_iter()
        .map(|(id, username, role, created_at, totp_enabled, email, email_notifications)| {
            let user_groups = state.db.get_user_groups(&id);
            serde_json::json!({
                "id": id,
                "username": username,
                "role": role,
                "totp_enabled": totp_enabled,
                "created_at": created_at,
                "email": email,
                "email_notifications": email_notifications,
                "groups": user_groups.iter().map(|(gid, gname, gcolor)| serde_json::json!({"id": gid, "name": gname, "color": gcolor})).collect::<Vec<_>>(),
            })
        })
        .collect();

    Json(ApiResponse::ok(users))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateUserRequest>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    if !has_permission(&state, &claims, "action.user_management") {
        return Json(ApiResponse::err("Keine Berechtigung"));
    }

    if req.username.len() < 3 || req.password.len() < 6 {
        return Json(ApiResponse::err("Benutzername min. 3 Zeichen, Passwort min. 6 Zeichen"));
    }

    let valid_roles = ["super_admin", "admin", "editor", "viewer", "user"];
    if !valid_roles.contains(&req.role.as_str()) {
        return Json(ApiResponse::err("Ungültige Rolle"));
    }

    let password_hash = match auth::hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => return Json(ApiResponse::err("Fehler beim Hashen")),
    };

    let user_id = uuid::Uuid::new_v4().to_string();
    match state.db.create_user_with_role(&user_id, &req.username, &password_hash, &req.role) {
        Ok(_) => {
            // Auto-assign to matching default group based on role
            let group_name = match req.role.as_str() {
                "super_admin" | "admin" => "Admin",
                "editor" => "Editor",
                "viewer" => "Viewer",
                "user" => "DockPit",
                _ => "",
            };
            if !group_name.is_empty() {
                let groups = state.db.list_groups();
                if let Some((gid, _, _, _, _, _)) = groups.iter().find(|(_, name, _, _, _, _)| name == group_name) {
                    state.db.set_user_groups(&user_id, &[*gid]);
                }
            }
            state.db.log_audit(&audit_user(&headers), "user_create", Some(&req.username), Some(&req.role));
            Json(ApiResponse::ok("Benutzer erstellt".to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    if !has_permission(&state, &claims, "action.user_management") {
        return Json(ApiResponse::err("Keine Berechtigung"));
    }

    if let Some(ref new_role) = req.role {
        let valid_roles = ["super_admin", "admin", "editor", "viewer", "user"];
        if !valid_roles.contains(&new_role.as_str()) {
            return Json(ApiResponse::err("Ungültige Rolle"));
        }
        if let Err(e) = state.db.update_user_role(&id, new_role) {
            return Json(ApiResponse::err(format!("Fehler: {}", e)));
        }
        // Auto-update group assignment to match new role
        let group_name = match new_role.as_str() {
            "super_admin" | "admin" => "Admin",
            "editor" => "Editor",
            "viewer" => "Viewer",
            "user" => "DockPit",
            _ => "",
        };
        if !group_name.is_empty() {
            let groups = state.db.list_groups();
            if let Some((gid, _, _, _, _, _)) = groups.iter().find(|(_, name, _, _, _, _)| name == group_name) {
                state.db.set_user_groups(&id, &[*gid]);
            }
        }
    }

    if let Some(ref new_password) = req.password {
        if new_password.len() < 6 {
            return Json(ApiResponse::err("Passwort muss min. 6 Zeichen haben"));
        }
        let new_hash = match auth::hash_password(new_password) {
            Ok(h) => h,
            Err(_) => return Json(ApiResponse::err("Fehler beim Hashen")),
        };
        // We need the username to update the password
        // Find user by id
        let users = state.db.list_users();
        let user = users.iter().find(|(uid, _, _, _, _, _, _)| uid == &id);
        match user {
            Some((_, username, _, _, _, _, _)) => {
                if let Err(e) = state.db.update_password(username, &new_hash) {
                    return Json(ApiResponse::err(format!("Fehler: {}", e)));
                }
            }
            None => return Json(ApiResponse::err("Benutzer nicht gefunden")),
        }
    }

    Json(ApiResponse::ok("Benutzer aktualisiert".to_string()))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    if !has_permission(&state, &claims, "action.user_management") {
        return Json(ApiResponse::err("Keine Berechtigung"));
    }

    // Cannot delete self
    if claims.sub == id {
        return Json(ApiResponse::err("Eigenen Account kann nicht gelöscht werden"));
    }

    match state.db.delete_user(&id) {
        Ok(_) => {
            state.db.log_audit(&audit_user(&headers), "user_delete", Some(&id), None);
            Json(ApiResponse::ok("Benutzer gelöscht".to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn reset_user_mfa(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    // Find username by id
    let users = state.db.list_users();
    let user = users.iter().find(|(uid, ..)| uid == &id);
    match user {
        Some((_, username, ..)) => {
            state.db.set_totp_secret(username, None).ok();
            Json(ApiResponse::ok("2FA zurückgesetzt".to_string()))
        }
        None => Json(ApiResponse::err("Benutzer nicht gefunden")),
    }
}

// === Settings ===

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<SettingsMap>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    if !has_permission(&state, &claims, "page.settings") {
        return Json(ApiResponse::err("Keine Berechtigung"));
    }

    let settings_vec = state.db.get_all_settings();
    let settings = settings_vec.into_iter().collect();

    Json(ApiResponse::ok(SettingsMap { settings }))
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SettingsMap>,
) -> Json<ApiResponse<String>> {
    let claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };
    if !has_permission(&state, &claims, "page.settings") {
        return Json(ApiResponse::err("Keine Berechtigung"));
    }

    for (key, value) in &req.settings {
        if let Err(e) = state.db.set_setting(key, value) {
            return Json(ApiResponse::err(format!("Fehler beim Speichern: {}", e)));
        }
    }

    state.db.log_audit(&audit_user(&headers), "settings_update", None, None);
    Json(ApiResponse::ok("Einstellungen gespeichert".to_string()))
}

pub async fn test_webhook(
    headers: axum::http::HeaderMap,
    Json(req): Json<WebhookTestRequest>,
) -> Json<ApiResponse<String>> {
    let _claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let payload = serde_json::json!({
        "text": "DockPit Test-Nachricht: Webhook funktioniert!",
        "source": "dockpit",
        "type": "test"
    });

    match client.post(&req.url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            Json(ApiResponse::ok("Webhook-Test erfolgreich".to_string()))
        }
        Ok(resp) => {
            Json(ApiResponse::err(format!("Webhook-Fehler: HTTP {}", resp.status())))
        }
        Err(e) => {
            Json(ApiResponse::err(format!("Webhook nicht erreichbar: {}", e)))
        }
    }
}

// === Update Monitor ===

pub async fn get_update_report(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<Vec<UpdateCheckResult>>> {
    let _claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    let results = state.db.get_latest_update_checks();
    Json(ApiResponse::ok(results))
}

pub async fn run_update_check(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<String>> {
    use std::sync::atomic::Ordering;

    // Check if already running
    if state.update_check_running.load(Ordering::SeqCst) {
        return Json(ApiResponse::err("Update-Check läuft bereits"));
    }

    state.db.log_audit(&audit_user(&headers), "update_check", None, Some("Manual check started"));

    // Mark as running and spawn background task
    state.update_check_running.store(true, Ordering::SeqCst);
    let state_clone = state.clone();

    tokio::spawn(async move {
        tracing::info!("Background update check started");
        state_clone.db.clear_update_checks().ok();

        let envs = state_clone.db.get_environments();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap();

        for env in &envs {
            let containers: Vec<ContainerInfo> = if env.is_local {
                state_clone.docker.list_containers().await.unwrap_or_default()
            } else {
                let url = format!("{}/api/containers", env.url);
                match client.get(&url)
                    .header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
                    .send().await
                {
                    Ok(resp) => resp.json::<ApiResponse<Vec<ContainerInfo>>>().await
                        .ok().and_then(|r| r.data).unwrap_or_default(),
                    Err(_) => continue,
                }
            };

            // Check all containers in parallel (much faster than sequential)
            let running: Vec<_> = containers.iter().filter(|c| c.state == "running").collect();
            let creds: Vec<_> = state_clone.db.get_all_registry_credentials()
                .into_iter()
                .map(|(r, u, p)| serde_json::json!({"registry": r, "username": u, "password": p}))
                .collect();
            let creds_json = serde_json::json!({ "credentials": creds });

            let mut handles = Vec::new();
            for container in &running {
                let state_ref = state_clone.clone();
                let env_ref = env.clone();
                let client_ref = client.clone();
                let creds_body = creds_json.clone();
                let cname = container.name.clone();
                let cimage = container.image.clone();
                let cid_str = container.id.clone();

                handles.push(tokio::spawn(async move {
                    let check = if env_ref.is_local {
                        state_ref.docker.check_image_update(&cimage).await
                            .ok().map(|(o, c, l)| (o, c, l, cimage.clone()))
                    } else {
                        let url = format!("{}/api/containers/{}/check-update", env_ref.url, cid_str);
                        match client_ref.post(&url)
                            .header("X-Agent-Token", env_ref.agent_token.as_deref().unwrap_or(""))
                            .json(&creds_body).send().await
                        {
                            Ok(resp) => resp.json::<ApiResponse<ImageUpdateCheck>>().await
                                .ok().and_then(|r| r.data)
                                .map(|c| (c.outdated, c.current_id, c.latest_id, c.image)),
                            Err(_) => None,
                        }
                    };
                    (cname, check)
                }));
            }

            for handle in handles {
                if let Ok((cname, Some((outdated, cid, lid, image)))) = handle.await {
                    state_clone.db.save_update_check(
                        &cname, &image, &env.name, &env.id,
                        outdated, Some(&cid), Some(&lid),
                    ).ok();
                }
            }
        }

        state_clone.update_check_running.store(false, Ordering::SeqCst);
        tracing::info!("Background update check completed");

        // Create notification
        let results = state_clone.db.get_latest_update_checks();
        let total = results.len();
        let outdated_count = results.iter().filter(|r| r.outdated).count();
        if outdated_count > 0 {
            let names: Vec<_> = results.iter().filter(|r| r.outdated).take(5).map(|r| r.container_name.clone()).collect();
            notify_and_email(&state_clone,
                "update_available",
                &format!("{} updates available", outdated_count),
                &format!("{} of {} containers have updates: {}", outdated_count, total, names.join(", ")),
            );
        } else {
            notify_and_email(&state_clone,
                "update_current",
                "All containers up to date",
                &format!("{} containers checked, all current", total),
            );
        }

        // Send webhook notification if configured
        let webhook_url = state_clone.db.get_setting("webhook_url");
        let webhook_enabled = state_clone.db.get_setting("webhook_enabled");
        if webhook_enabled.as_deref() == Some("true") {
            if let Some(url) = webhook_url {
                let results = state_clone.db.get_latest_update_checks();
                let outdated: Vec<_> = results.iter().filter(|r| r.outdated).collect();
                let msg = if outdated.is_empty() {
                    "DockPit Update-Check: Alle Container sind aktuell.".to_string()
                } else {
                    let names: Vec<_> = outdated.iter().map(|r| format!("• {} ({})", r.container_name, r.image)).collect();
                    format!("DockPit Update-Check: {} Updates verfügbar:\n{}", outdated.len(), names.join("\n"))
                };
                let payload = serde_json::json!({ "text": msg, "source": "dockpit", "type": "update_report" });
                let _ = reqwest::Client::new().post(&url).json(&payload).send().await;
            }
        }
    });

    Json(ApiResponse::ok("Update-Check gestartet (läuft im Hintergrund)".to_string()))
}

pub async fn get_update_check_status(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<UpdateCheckStatus>> {
    use std::sync::atomic::Ordering;

    let running = state.update_check_running.load(Ordering::SeqCst);
    let results = state.db.get_latest_update_checks();
    let outdated = results.iter().filter(|r| r.outdated).count();
    let last_check = results.first().map(|r| r.checked_at.clone());

    Json(ApiResponse::ok(UpdateCheckStatus {
        running,
        total_checked: results.len(),
        total_outdated: outdated,
        last_check,
    }))
}

pub async fn clear_update_report(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<String>> {
    let _claims = match extract_claims(&headers) {
        Some(c) => c,
        None => return Json(ApiResponse::err("Nicht autorisiert")),
    };

    match state.db.clear_update_checks() {
        Ok(_) => Json(ApiResponse::ok("Ergebnisse gelöscht".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

// === Home: All Servers Overview ===

#[derive(serde::Serialize)]
pub struct ServerOverview {
    pub id: String,
    pub name: String,
    pub url: String,
    pub is_local: bool,
    pub info: SystemInfo,
}

pub async fn home_servers(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<ServerOverview>>> {
    let envs = state.db.get_environments();

    // Return basic info instantly, frontend fetches detailed info per server
    let mut servers = Vec::new();
    for env in envs {
        let info = if env.is_local {
            // Local is fast - get system info directly
            state.docker.get_system_info().await
        } else {
            SystemInfo { status: "loading".into(), hostname: env.name.clone(), ..Default::default() }
        };
        servers.push(ServerOverview {
            id: env.id,
            name: env.name,
            url: if env.is_local { "local".into() } else { env.url },
            is_local: env.is_local,
            info,
        });
    }

    Json(ApiResponse::ok(servers))
}

// === Environment System Info ===

pub async fn env_system_info(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<SystemInfo>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        Json(ApiResponse::ok(state.docker.get_system_info().await))
    } else {
        agent_get(&env, "/api/system").await
    }
}

// === Disk Usage ===

pub async fn env_disk_usage(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<DiskUsageInfo>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    if env.is_local {
        Json(ApiResponse::ok(state.docker.get_disk_usage().await))
    } else {
        agent_get(&env, "/api/disk-usage").await
    }
}

// === Dashboard ===

pub async fn dashboard(State(state): State<Arc<AppState>>) -> Json<ApiResponse<DashboardStats>> {
    let mut stats = state.docker.get_dashboard_stats().await;
    let mut envs = state.db.get_environments();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    // Check remote environments in parallel
    let mut handles = Vec::new();
    for (i, env) in envs.iter().enumerate() {
        if !env.is_local {
            let url = format!("{}/health", env.url);
            let c = client.clone();
            handles.push((i, tokio::spawn(async move {
                match c.get(&url).send().await {
                    Ok(r) if r.status().is_success() => "online".to_string(),
                    _ => "offline".to_string(),
                }
            })));
        }
    }
    for env in &mut envs {
        if env.is_local { env.status = "online".to_string(); }
    }
    for (i, handle) in handles {
        envs[i].status = handle.await.unwrap_or_else(|_| "offline".to_string());
    }

    stats.environments = envs;
    Json(ApiResponse::ok(stats))
}

// === Environment-scoped Stats ===

pub async fn env_stats(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<EnvStats>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        let s = state.docker.get_dashboard_stats().await;
        Json(ApiResponse::ok(EnvStats {
            containers_running: s.containers_running,
            containers_stopped: s.containers_stopped,
            containers_total: s.containers_total,
            images_total: s.images_total,
            volumes_total: s.volumes_total,
            networks_total: s.networks_total,
        }))
    } else {
        // Aggregate from agent endpoints
        let c: Json<ApiResponse<Vec<ContainerInfo>>> = agent_get(&env, "/api/containers").await;
        let i: Json<ApiResponse<Vec<ImageInfo>>> = agent_get(&env, "/api/images").await;
        let v: Json<ApiResponse<Vec<VolumeInfo>>> = agent_get(&env, "/api/volumes").await;
        let n: Json<ApiResponse<Vec<NetworkInfo>>> = agent_get(&env, "/api/networks").await;

        let containers = c.0.data.unwrap_or_default();
        let running = containers.iter().filter(|c| c.state == "running").count();

        Json(ApiResponse::ok(EnvStats {
            containers_running: running,
            containers_stopped: containers.len() - running,
            containers_total: containers.len(),
            images_total: i.0.data.map(|d| d.len()).unwrap_or(0),
            volumes_total: v.0.data.map(|d| d.len()).unwrap_or(0),
            networks_total: n.0.data.map(|d| d.len()).unwrap_or(0),
        }))
    }
}

// === Environment-scoped Containers ===

pub async fn env_containers(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<ContainerInfo>>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.list_containers().await {
            Ok(c) => Json(ApiResponse::ok(c)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_get(&env, "/api/containers").await
    }
}

pub async fn env_container_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, container_id)): Path<(String, String)>,
    Json(action): Json<ContainerAction>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    // Get container name for audit log
    let container_name = if env.is_local {
        state.docker.inspect_container(&container_id).await.ok()
            .and_then(|c| c.name).map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_else(|| container_id[..12.min(container_id.len())].to_string())
    } else { container_id[..12.min(container_id.len())].to_string() };
    let server = env_name(&state, &env_id);
    state.db.log_audit(&audit_user(&headers), &format!("container_{}", action.action), Some(&container_name), Some(&format!("Server: {}", server)));

    if env.is_local {
        let result = match action.action.as_str() {
            "start" => state.docker.start_container(&container_id).await,
            "stop" => state.docker.stop_container(&container_id).await,
            "restart" => state.docker.restart_container(&container_id).await,
            "remove" => state.docker.remove_container(&container_id).await,
            _ => return Json(ApiResponse::err("Ungültige Aktion")),
        };
        match result {
            Ok(_) => Json(ApiResponse::ok(format!("{} erfolgreich", action.action))),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        let path = format!("/api/containers/{}/action", container_id);
        agent_post(&env, &path, &action).await
    }
}

pub async fn env_container_logs(
    State(state): State<Arc<AppState>>,
    Path((env_id, container_id)): Path<(String, String)>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    let tail: usize = query.get("tail").and_then(|t| t.parse().ok()).unwrap_or(200);

    if env.is_local {
        match state.docker.container_logs(&container_id, tail).await {
            Ok(logs) => Json(ApiResponse::ok(logs)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        let path = format!("/api/containers/{}/logs?tail={}", container_id, tail);
        agent_get(&env, &path).await
    }
}

pub async fn env_check_container_update(
    State(state): State<Arc<AppState>>,
    Path((env_id, container_id)): Path<(String, String)>,
) -> Json<ApiResponse<ImageUpdateCheck>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        let container = match state.docker.inspect_container(&container_id).await {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Container nicht gefunden: {}", e))),
        };
        let image_name = container.config.and_then(|c| c.image).unwrap_or_default();

        match state.docker.check_image_update(&image_name).await {
            Ok((outdated, current_id, latest_id)) => Json(ApiResponse::ok(ImageUpdateCheck {
                outdated, current_id, latest_id, image: image_name,
            })),
            Err(e) => Json(ApiResponse::err(e)),
        }
    } else {
        // Send registry credentials to agent so it can check private repos
        let creds: Vec<_> = state.db.get_all_registry_credentials()
            .into_iter()
            .map(|(registry, username, password)| serde_json::json!({ "registry": registry, "username": username, "password": password }))
            .collect();
        let body = serde_json::json!({ "credentials": creds });

        let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().unwrap();
        let url = format!("{}/api/containers/{}/check-update", env.url, container_id);
        match client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).json(&body).send().await {
            Ok(resp) => match resp.json::<ApiResponse<ImageUpdateCheck>>().await { Ok(d) => Json(d), Err(e) => Json(ApiResponse::err(e.to_string())) },
            Err(e) => Json(ApiResponse::err(format!("Agent: {}", e))),
        }
    }
}

pub async fn env_recreate_container(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, container_id)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    let server = env_name(&state, &env_id);

    if env.is_local {
        // Check if it's a stack container
        let inspect = match state.docker.inspect_container(&container_id).await {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        };
        let cname = inspect.name.as_ref().map(|n| n.trim_start_matches('/').to_string()).unwrap_or_else(|| container_id[..12.min(container_id.len())].to_string());
        state.db.log_audit(&audit_user(&headers), "container_recreate", Some(&cname), Some(&format!("Server: {}", server)));

        // Save snapshot before recreate
        let snap_image = inspect.config.as_ref().and_then(|c| c.image.clone()).unwrap_or_default();
        let snap_json = serde_json::to_string(&inspect).unwrap_or_default();
        tracing::info!("Saving snapshot for '{}' (image: {}, json_len: {})", cname, snap_image, snap_json.len());
        state.db.save_container_snapshot(&env_id, &cname, &snap_image, &snap_json);

        let stack_name = inspect.config.as_ref()
            .and_then(|c| c.labels.as_ref())
            .and_then(|l| l.get("com.docker.compose.project"))
            .cloned();
        let service_name = inspect.config.as_ref()
            .and_then(|c| c.labels.as_ref())
            .and_then(|l| l.get("com.docker.compose.service"))
            .cloned();

        if let (Some(stack), Some(service)) = (stack_name, service_name) {
            let dir = state.stacks.stack_dir_by_name(&stack);
            if let Some(dir) = dir {
                let mut output_lines = Vec::new();
                output_lines.push(format!("Stack: {}, Service: {}", stack, service));
                output_lines.push(String::new());

                // Pull
                output_lines.push("→ docker compose pull...".to_string());
                if let Ok(pull_out) = tokio::process::Command::new("docker")
                    .args(["compose", "pull", &service])
                    .current_dir(&dir).output().await
                {
                    let stdout = String::from_utf8_lossy(&pull_out.stdout);
                    let stderr = String::from_utf8_lossy(&pull_out.stderr);
                    for line in stdout.lines().chain(stderr.lines()) {
                        if !line.trim().is_empty() { output_lines.push(format!("  {}", line)); }
                    }
                }

                // Up
                output_lines.push(String::new());
                output_lines.push("→ docker compose up --force-recreate...".to_string());
                match tokio::process::Command::new("docker")
                    .args(["compose", "up", "-d", "--force-recreate", &service])
                    .current_dir(&dir).output().await
                {
                    Ok(up_out) => {
                        let stdout = String::from_utf8_lossy(&up_out.stdout);
                        let stderr = String::from_utf8_lossy(&up_out.stderr);
                        for line in stdout.lines().chain(stderr.lines()) {
                            if !line.trim().is_empty() { output_lines.push(format!("  {}", line)); }
                        }
                        if up_out.status.success() {
                            state.db.mark_container_updated(&cname);
                            return Json(ApiResponse::ok(output_lines.join("\n")));
                        } else {
                            return Json(ApiResponse::err(output_lines.join("\n")));
                        }
                    }
                    Err(e) => return Json(ApiResponse::err(format!("{}\n\nFehler: {}", output_lines.join("\n"), e))),
                }
            }
        }

        // Standalone container: use Docker API to recreate (snapshot already saved above)
        match state.docker.recreate_container(&container_id).await {
            Ok((msg, _snap_name, _snap_json)) => {
                state.db.mark_container_updated(&cname);
                Json(ApiResponse::ok(msg))
            }
            Err(e) => Json(ApiResponse::err(e)),
        }
    } else {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().unwrap();

        // Save snapshot from remote agent before recreate
        let inspect_url = format!("{}/api/containers/{}/inspect", env.url, container_id);
        if let Ok(resp) = client.get(&inspect_url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
            if let Ok(inspect_resp) = resp.json::<ApiResponse<serde_json::Value>>().await {
                if let Some(inspect_data) = inspect_resp.data {
                    let snap_name = inspect_data.get("Name").and_then(|n| n.as_str()).unwrap_or("").trim_start_matches('/').to_string();
                    let snap_image = inspect_data.get("Config").and_then(|c| c.get("Image")).and_then(|i| i.as_str()).unwrap_or("").to_string();
                    let snap_json = serde_json::to_string(&inspect_data).unwrap_or_default();
                    if !snap_name.is_empty() {
                        tracing::info!("Saving remote snapshot for '{}' (image: {})", snap_name, snap_image);
                        state.db.save_container_snapshot(&env_id, &snap_name, &snap_image, &snap_json);
                        state.db.log_audit(&audit_user(&headers), "container_recreate", Some(&snap_name), Some(&format!("Server: {}", server)));
                    }
                }
            }
        }

        let url = format!("{}/api/containers/{}/recreate", env.url, container_id);
        match client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).json(&()).send().await {
            Ok(resp) => match resp.json::<ApiResponse<String>>().await {
                Ok(d) => {
                    if d.success {
                        // Try to resolve container name for update check cleanup
                        let name_url = format!("{}/api/containers", env.url);
                        if let Ok(r) = client.get(&name_url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
                            if let Ok(containers) = r.json::<ApiResponse<Vec<ContainerInfo>>>().await {
                                if let Some(c) = containers.data.as_ref().and_then(|cs| cs.iter().find(|c| c.id.starts_with(&container_id) || c.id == container_id)) {
                                    state.db.mark_container_updated(&c.name);
                                }
                            }
                        }
                    }
                    Json(d)
                }
                Err(e) => Json(ApiResponse::err(e.to_string())),
            },
            Err(e) => Json(ApiResponse::err(format!("Agent: {}", e))),
        }
    }
}

// === Container Migration ===

#[derive(Debug, serde::Deserialize)]
pub struct MigrateRequest {
    pub target_env_id: String,
    pub stop_source: bool,
}

pub async fn env_migrate_container(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, container_id)): Path<(String, String)>,
    Json(req): Json<MigrateRequest>,
) -> Json<ApiResponse<String>> {
    let source_env = match state.db.get_environment(&env_id) {
        Some(e) => e,
        None => return Json(ApiResponse::err("Quell-Umgebung nicht gefunden")),
    };
    let target_env = match state.db.get_environment(&req.target_env_id) {
        Some(e) => e,
        None => return Json(ApiResponse::err("Ziel-Umgebung nicht gefunden")),
    };

    if env_id == req.target_env_id {
        return Json(ApiResponse::err("Quell- und Ziel-Umgebung sind identisch"));
    }

    // Step 1: Get container details from source
    let container_info: serde_json::Value = if source_env.is_local {
        match state.docker.inspect_container_raw(&container_id).await {
            Ok(info) => info,
            Err(e) => return Json(ApiResponse::err(format!("Container-Details nicht abrufbar: {}", e))),
        }
    } else {
        let r: ApiResponse<serde_json::Value> = match agent_get(&source_env, &format!("/api/containers/{}/inspect", container_id)).await {
            Json(resp) => resp,
        };
        match r.data {
            Some(d) => d,
            None => return Json(ApiResponse::err(r.error.unwrap_or_else(|| "Container nicht gefunden".into()))),
        }
    };

    // Extract image name from container info
    let image = container_info
        .get("Config")
        .and_then(|c| c.get("Image"))
        .and_then(|i| i.as_str())
        .unwrap_or("")
        .to_string();

    if image.is_empty() {
        return Json(ApiResponse::err("Image-Name konnte nicht ermittelt werden"));
    }

    // Extract container name
    let container_name = container_info
        .get("Name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .trim_start_matches('/')
        .to_string();

    // Step 1.5: Propagate registry credentials to target before pull
    if !target_env.is_local {
        let registries = state.db.get_all_registry_credentials();
        let login_client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
        for (registry, username, password) in &registries {
            let url = format!("{}/api/docker/login", target_env.url);
            let body = serde_json::json!({ "registry": registry, "username": username, "password": password });
            login_client.post(&url)
                .header("X-Agent-Token", target_env.agent_token.as_deref().unwrap_or(""))
                .json(&body)
                .send().await.ok();
        }
    }

    // Step 2: Pull image on target
    let pull_body = serde_json::json!({ "image": image });
    if target_env.is_local {
        if let Err(e) = state.docker.pull_image(&image).await {
            return Json(ApiResponse::err(format!("Image Pull auf Ziel fehlgeschlagen: {}", e)));
        }
    } else {
        let r: Json<ApiResponse<String>> = agent_post(&target_env, "/api/images/pull", &pull_body).await;
        if !r.0.success {
            return Json(ApiResponse::err(format!("Image Pull auf Ziel fehlgeschlagen: {}", r.0.error.unwrap_or_default())));
        }
    }

    // Step 3: Stop source container if requested
    if req.stop_source {
        if source_env.is_local {
            state.docker.stop_container(&container_id).await.ok();
        } else {
            let stop_body = serde_json::json!({ "action": "stop" });
            let _: Json<ApiResponse<String>> = agent_post(&source_env, &format!("/api/containers/{}/action", container_id), &stop_body).await;
        }
    }

    let source_name = source_env.name.clone();
    let target_name = target_env.name.clone();
    state.db.log_audit(
        &audit_user(&headers),
        "container_migrate",
        Some(&container_name),
        Some(&format!("{} → {}", source_name, target_name)),
    );

    Json(ApiResponse::ok(format!(
        "Image '{}' auf {} bereitgestellt{}",
        image,
        target_name,
        if req.stop_source { format!(". Container auf {} gestoppt", source_name) } else { String::new() }
    )))
}

// === Container Inspect ===

pub async fn env_inspect_container(
    State(state): State<Arc<AppState>>,
    Path((env_id, container_id)): Path<(String, String)>,
) -> Json<ApiResponse<serde_json::Value>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    if env.is_local {
        match state.docker.inspect_container_raw(&container_id).await {
            Ok(data) => Json(ApiResponse::ok(data)),
            Err(e) => Json(ApiResponse::err(e)),
        }
    } else {
        agent_get(&env, &format!("/api/containers/{}/inspect", container_id)).await
    }
}

// === Container Rollback ===

pub async fn get_container_snapshots(
    State(state): State<Arc<AppState>>,
    Path(container_name): Path<String>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let snapshots = state.db.get_container_snapshots(&container_name);
    let result: Vec<serde_json::Value> = snapshots.iter().map(|(id, _env_id, image, _json, created_at)| {
        serde_json::json!({
            "id": id,
            "image": image,
            "created_at": created_at,
        })
    }).collect();
    Json(ApiResponse::ok(result))
}

pub async fn rollback_container(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, container_id)): Path<(String, String)>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let snapshot_id = match req.get("snapshot_id").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => return Json(ApiResponse::err("snapshot_id fehlt")),
    };

    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    let (container_name, image, config_json) = match state.db.get_snapshot_by_id(snapshot_id) {
        Some(s) => s,
        None => return Json(ApiResponse::err("Snapshot nicht gefunden")),
    };

    state.db.log_audit(&audit_user(&headers), "container_rollback", Some(&container_name), Some(&format!("→ {}", image)));

    if env.is_local {
        match state.docker.rollback_container(&container_id, &config_json).await {
            Ok(msg) => Json(ApiResponse::ok(msg)),
            Err(e) => Json(ApiResponse::err(e)),
        }
    } else {
        // Remote: send snapshot JSON to agent's rollback endpoint
        let client = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().unwrap();
        let url = format!("{}/api/containers/{}/rollback", env.url, container_id);
        match client.post(&url)
            .header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
            .header("Content-Type", "application/json")
            .body(config_json)
            .send().await
        {
            Ok(resp) => match resp.json::<ApiResponse<String>>().await {
                Ok(d) => Json(d),
                Err(e) => Json(ApiResponse::err(format!("Agent: {}", e))),
            },
            Err(e) => Json(ApiResponse::err(format!("Agent: {}", e))),
        }
    }
}

pub async fn delete_snapshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<String>> {
    match state.db.delete_snapshot(id) {
        Ok(_) => Json(ApiResponse::ok("Snapshot gelöscht".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

// === Shell Snippets ===

pub async fn get_snippets(
    State(state): State<Arc<AppState>>,
    Path(container_name): Path<String>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let snippets = state.db.get_snippets(&container_name);
    let result: Vec<serde_json::Value> = snippets.iter().map(|(id, title, command, created_at)| {
        serde_json::json!({ "id": id, "title": title, "command": command, "created_at": created_at })
    }).collect();
    Json(ApiResponse::ok(result))
}

pub async fn create_snippet(
    State(state): State<Arc<AppState>>,
    Path(container_name): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let title = req.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let command = req.get("command").and_then(|v| v.as_str()).unwrap_or("");
    if title.is_empty() || command.is_empty() {
        return Json(ApiResponse::err("Titel und Befehl erforderlich"));
    }
    match state.db.save_snippet(&container_name, title, command) {
        Ok(_) => Json(ApiResponse::ok("Snippet gespeichert".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn delete_snippet_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<String>> {
    match state.db.delete_snippet(id) {
        Ok(_) => Json(ApiResponse::ok("Snippet gelöscht".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

// === Alert Rules ===

pub async fn get_alert_rules(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let rules = state.db.get_alert_rules();
    let result: Vec<serde_json::Value> = rules.iter().map(|(id, name, enabled, event_match, action_type, config_json, last_triggered, trigger_count)| {
        serde_json::json!({
            "id": id, "name": name, "enabled": enabled,
            "event_match": event_match, "action_type": action_type,
            "config_json": config_json, "last_triggered": last_triggered,
            "trigger_count": trigger_count,
        })
    }).collect();
    Json(ApiResponse::ok(result))
}

pub async fn create_alert_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let event_match = req.get("event_match").and_then(|v| v.as_str()).unwrap_or("");
    let action_type = req.get("action_type").and_then(|v| v.as_str()).unwrap_or("");
    if name.is_empty() || event_match.is_empty() || action_type.is_empty() {
        return Json(ApiResponse::err("Name, Event und Aktion erforderlich"));
    }
    let config_json = req.get("config_json").and_then(|v| v.as_str());
    match state.db.create_alert_rule(name, event_match, action_type, config_json) {
        Ok(_) => Json(ApiResponse::ok("Regel erstellt".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn toggle_alert_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let enabled = req.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    match state.db.update_alert_rule_enabled(id, enabled) {
        Ok(_) => Json(ApiResponse::ok("Aktualisiert".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn delete_alert_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<String>> {
    match state.db.delete_alert_rule(id) {
        Ok(_) => Json(ApiResponse::ok("Regel gelöscht".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn evaluate_alert_rules(state: Arc<AppState>) {
    let rules = state.db.get_alert_rules();
    if rules.is_empty() { return; }

    let events = state.db.get_recent_event_actions(60);

    for (id, name, enabled, event_match, action_type, _config_json, _last_triggered, _trigger_count) in &rules {
        if !enabled { continue; }

        for (_env_id, container_id, container_name, event_action) in &events {
            let matches = match event_match.as_str() {
                "container_stop" => event_action == "stop" || event_action == "die",
                "container_oom" => event_action == "oom",
                "container_restart_loop" => event_action == "restart",
                _ => event_action == event_match,
            };

            if !matches { continue; }

            match action_type.as_str() {
                "restart" => {
                    if let Some(cid) = container_id {
                        tokio::process::Command::new("docker").args(["start", cid]).output().await.ok();
                        tracing::info!("Auto-fix: Started container {} (rule: {})", container_name.as_deref().unwrap_or("?"), name);
                        state.db.mark_alert_triggered(*id);
                        notify_and_email(&state, "alert_autofix", &format!("Auto-Fix: {}", name), &format!("Container {} automatically restarted", container_name.as_deref().unwrap_or("?")));
                    }
                }
                "notify" => {
                    notify_and_email(&state, "alert", &format!("Alert: {}", name), &format!("Event '{}' for container {}", event_action, container_name.as_deref().unwrap_or("?")));
                    state.db.mark_alert_triggered(*id);
                }
                "prune" => {
                    tokio::process::Command::new("docker").args(["system", "prune", "-f"]).output().await.ok();
                    state.db.mark_alert_triggered(*id);
                    notify_and_email(&state, "alert_autofix", &format!("Auto-Fix: {}", name), "Docker System Prune executed");
                }
                _ => {}
            }
        }
    }
}

// === Container Diff ===

pub async fn get_snapshot_diff(
    State(state): State<Arc<AppState>>,
    Path((id1, id2)): Path<(i64, i64)>,
) -> Json<ApiResponse<serde_json::Value>> {
    let snap1 = match state.db.get_snapshot_by_id(id1) {
        Some(s) => s,
        None => return Json(ApiResponse::err("Snapshot 1 nicht gefunden")),
    };
    let snap2 = match state.db.get_snapshot_by_id(id2) {
        Some(s) => s,
        None => return Json(ApiResponse::err("Snapshot 2 nicht gefunden")),
    };

    let (name1, image1, json1) = snap1;
    let (_name2, image2, json2) = snap2;

    let v1: serde_json::Value = serde_json::from_str(&json1).unwrap_or_default();
    let v2: serde_json::Value = serde_json::from_str(&json2).unwrap_or_default();

    let mut changes = Vec::new();

    // Image diff
    if image1 != image2 {
        changes.push(serde_json::json!({ "field": "Image", "old": image1, "new": image2 }));
    }

    // Image ID diff
    let id1_val = v1.get("Image").and_then(|v| v.as_str()).unwrap_or("");
    let id2_val = v2.get("Image").and_then(|v| v.as_str()).unwrap_or("");
    if id1_val != id2_val {
        changes.push(serde_json::json!({ "field": "Image ID", "old": &id1_val[..std::cmp::min(id1_val.len(), 19)], "new": &id2_val[..std::cmp::min(id2_val.len(), 19)] }));
    }

    // Env vars diff
    let env1: Vec<&str> = v1.pointer("/Config/Env").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    let env2: Vec<&str> = v2.pointer("/Config/Env").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    for e in &env1 {
        if !env2.contains(e) {
            let key = e.split('=').next().unwrap_or(e);
            let in_new = env2.iter().any(|o| o.split('=').next() == Some(key));
            if !in_new {
                changes.push(serde_json::json!({ "field": "Env removed", "old": key, "new": "" }));
            }
        }
    }
    for e in &env2 {
        if !env1.contains(e) {
            let key = e.split('=').next().unwrap_or(e);
            let in_old = env1.iter().any(|o| o.split('=').next() == Some(key));
            if in_old {
                changes.push(serde_json::json!({ "field": "Env changed", "old": key, "new": e.split('=').nth(1).unwrap_or("") }));
            } else {
                changes.push(serde_json::json!({ "field": "Env added", "old": "", "new": key }));
            }
        }
    }

    // Ports diff
    let ports1 = v1.pointer("/HostConfig/PortBindings").cloned().unwrap_or_default();
    let ports2 = v2.pointer("/HostConfig/PortBindings").cloned().unwrap_or_default();
    if ports1 != ports2 {
        changes.push(serde_json::json!({ "field": "Ports", "old": ports1.to_string(), "new": ports2.to_string() }));
    }

    // Volumes/Binds diff
    let binds1: Vec<&str> = v1.pointer("/HostConfig/Binds").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    let binds2: Vec<&str> = v2.pointer("/HostConfig/Binds").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    for b in &binds1 { if !binds2.contains(b) { changes.push(serde_json::json!({ "field": "Volume removed", "old": b, "new": "" })); } }
    for b in &binds2 { if !binds1.contains(b) { changes.push(serde_json::json!({ "field": "Volume added", "old": "", "new": b })); } }

    // Labels diff
    let labels1 = v1.pointer("/Config/Labels").cloned().unwrap_or_default();
    let labels2 = v2.pointer("/Config/Labels").cloned().unwrap_or_default();
    if labels1 != labels2 {
        if let (Some(l1), Some(l2)) = (labels1.as_object(), labels2.as_object()) {
            for (k, v) in l1 { if l2.get(k) != Some(v) { changes.push(serde_json::json!({ "field": "Label", "old": format!("{}={}", k, v), "new": l2.get(k).map(|v| v.to_string()).unwrap_or_else(|| "removed".into()) })); } }
            for (k, v) in l2 { if !l1.contains_key(k) { changes.push(serde_json::json!({ "field": "Label added", "old": "", "new": format!("{}={}", k, v) })); } }
        }
    }

    // Network diff
    let net1 = v1.pointer("/NetworkSettings/Networks").and_then(|v| v.as_object()).map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
    let net2 = v2.pointer("/NetworkSettings/Networks").and_then(|v| v.as_object()).map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
    if net1 != net2 {
        changes.push(serde_json::json!({ "field": "Networks", "old": net1.join(", "), "new": net2.join(", ") }));
    }

    Json(ApiResponse::ok(serde_json::json!({
        "container": name1,
        "snapshot1_image": image1,
        "snapshot2_image": image2,
        "changes": changes,
        "total_changes": changes.len(),
    })))
}

// === Stack Migration ===

#[derive(Debug, serde::Deserialize)]
pub struct StackMigrateRequest {
    pub target_env_id: String,
    pub stop_source: bool,
    pub deploy: bool,
}

pub async fn env_migrate_stack(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, name)): Path<(String, String)>,
    Json(req): Json<StackMigrateRequest>,
) -> Json<ApiResponse<String>> {
    let source_env = match state.db.get_environment(&env_id) {
        Some(e) => e,
        None => return Json(ApiResponse::err("Quell-Umgebung nicht gefunden")),
    };
    let target_env = match state.db.get_environment(&req.target_env_id) {
        Some(e) => e,
        None => return Json(ApiResponse::err("Ziel-Umgebung nicht gefunden")),
    };

    if env_id == req.target_env_id {
        return Json(ApiResponse::err("Quell- und Ziel-Umgebung sind identisch"));
    }

    // Step 1: Get stack details from source
    let stack_detail: StackDetail = if source_env.is_local {
        match state.stacks.get_stack_detail(&name).await {
            Ok(d) => d,
            Err(e) => return Json(ApiResponse::err(format!("Stack nicht gefunden: {}", e))),
        }
    } else {
        let r: Json<ApiResponse<StackDetail>> = agent_get(&source_env, &format!("/api/stacks/{}", name)).await;
        match r.0.data {
            Some(d) => d,
            None => return Json(ApiResponse::err(r.0.error.unwrap_or_else(|| "Stack nicht gefunden".into()))),
        }
    };

    // Step 2: Create stack on target
    let create_body = serde_json::json!({
        "name": name,
        "compose_content": stack_detail.compose_content,
        "env_content": stack_detail.env_content,
        "extra_files": stack_detail.extra_files,
    });

    if target_env.is_local {
        let create_req = crate::models::CreateStackRequest {
            name: name.clone(),
            compose_content: stack_detail.compose_content.clone(),
            env_content: stack_detail.env_content.clone(),
            extra_files: if stack_detail.extra_files.is_empty() { None } else { Some(stack_detail.extra_files.clone()) },
        };
        if let Err(e) = state.stacks.create_stack(&create_req) {
            return Json(ApiResponse::err(format!("Stack auf Ziel erstellen fehlgeschlagen: {}", e)));
        }
    } else {
        let r: Json<ApiResponse<String>> = agent_post(&target_env, "/api/stacks", &create_body).await;
        if !r.0.success {
            return Json(ApiResponse::err(format!("Stack auf Ziel erstellen fehlgeschlagen: {}", r.0.error.unwrap_or_default())));
        }
    }

    // Step 2.5: Propagate registry credentials to target before deploy
    if req.deploy && !target_env.is_local {
        let registries = state.db.get_all_registry_credentials();
        let login_client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
        for (registry, username, password) in &registries {
            let url = format!("{}/api/docker/login", target_env.url);
            let body = serde_json::json!({
                "registry": registry,
                "username": username,
                "password": password,
            });
            login_client.post(&url)
                .header("X-Agent-Token", target_env.agent_token.as_deref().unwrap_or(""))
                .json(&body)
                .send().await.ok();
        }
    }

    // Step 3: Deploy on target if requested
    if req.deploy {
        if target_env.is_local {
            if let Err(e) = state.stacks.deploy_stack(&name).await {
                return Json(ApiResponse::err(format!("Stack Deploy auf Ziel fehlgeschlagen: {}", e)));
            }
        } else {
            let r: Json<ApiResponse<String>> = agent_post(&target_env, &format!("/api/stacks/{}/deploy", name), &()).await;
            if !r.0.success {
                return Json(ApiResponse::err(format!("Stack Deploy auf Ziel fehlgeschlagen: {}", r.0.error.unwrap_or_default())));
            }
        }
    }

    // Step 4: Stop source stack if requested
    if req.stop_source {
        if source_env.is_local {
            state.stacks.stop_stack(&name).await.ok();
        } else {
            let _: Json<ApiResponse<String>> = agent_post(&source_env, &format!("/api/stacks/{}/stop", name), &()).await;
        }
    }

    let source_name = source_env.name.clone();
    let target_name = target_env.name.clone();
    state.db.log_audit(
        &audit_user(&headers),
        "stack_migrate",
        Some(&name),
        Some(&format!("{} → {}", source_name, target_name)),
    );

    Json(ApiResponse::ok(format!(
        "Stack '{}' nach {} migriert{}{}",
        name,
        target_name,
        if req.deploy { " und gestartet" } else { "" },
        if req.stop_source { format!(". Auf {} gestoppt", source_name) } else { String::new() }
    )))
}

// === Terminal (WebSocket exec) ===

#[derive(serde::Deserialize)]
pub struct TerminalQuery {
    pub token: String,
    pub shell: Option<String>,
    pub user: Option<String>,
}

/// Generate a one-time WebSocket token (valid 30 seconds, single use).
/// This endpoint is behind auth_middleware, so JWT is already validated.
pub async fn create_ws_token(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<String>> {
    let token = auth::generate_agent_token();
    {
        let mut tokens = state.ws_tokens.lock().unwrap();
        let now = std::time::Instant::now();
        tokens.retain(|_, created| now.duration_since(*created).as_secs() < 60);
        tokens.insert(token.clone(), now);
    }
    Json(ApiResponse::ok(token))
}

/// Validate and consume a one-time WS token. Falls back to JWT validation for backwards compat.
fn validate_ws_token(state: &AppState, token: &str) -> bool {
    // Try one-time token first
    {
        let mut tokens = state.ws_tokens.lock().unwrap();
        if let Some(created) = tokens.remove(token) {
            let age = std::time::Instant::now().duration_since(created).as_secs();
            return age < 30;
        }
    }
    // Fall back to JWT (backwards compatibility)
    auth::validate_token(token).is_ok()
}

pub async fn env_container_terminal(
    State(state): State<Arc<AppState>>,
    Path((env_id, container_id)): Path<(String, String)>,
    Query(query): Query<TerminalQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if !validate_ws_token(&state, &query.token) {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    let env = match state.db.get_environment(&env_id) {
        Some(e) => e,
        None => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };
    let shell = query.shell.unwrap_or_else(|| "/bin/sh".to_string());
    let user = query.user;

    if env.is_local {
        ws.on_upgrade(move |socket| handle_terminal(socket, state, container_id, shell, user))
    } else {
        // Proxy WebSocket to agent
        let agent_url = env.url.clone();
        let agent_token = env.agent_token.clone().unwrap_or_default();
        ws.on_upgrade(move |socket| proxy_terminal(socket, agent_url, agent_token, container_id, shell, user))
    }
}

async fn handle_terminal(
    mut socket: WebSocket,
    state: Arc<AppState>,
    container_id: String,
    shell: String,
    user: Option<String>,
) {
    use futures_util::{StreamExt, SinkExt};
    use bollard::exec::StartExecResults;
    use tokio::io::AsyncWriteExt;

    // Create exec session
    let cmd = vec![shell.as_str()];
    let exec_id = match state.docker.create_exec(&container_id, cmd, user.as_deref()).await {
        Ok(id) => id,
        Err(e) => {
            let msg = format!("Shell '{}' nicht verfügbar in diesem Container: {}", shell, e);
            tracing::error!("{}", msg);
            let _ = socket.send(Message::Text(format!("\x1b[31m{}\x1b[0m\r\n", msg).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    // Start exec
    let exec_result = match state.docker.start_exec(&exec_id).await {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("Exec fehlgeschlagen: {}", e);
            tracing::error!("{}", msg);
            let _ = socket.send(Message::Text(format!("\x1b[31m{}\x1b[0m\r\n", msg).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    match exec_result {
        StartExecResults::Attached { mut output, mut input } => {
            let (mut ws_tx, mut ws_rx) = socket.split();

            // Docker output → WebSocket
            let out_handle = tokio::spawn(async move {
                while let Some(Ok(log)) = output.next().await {
                    let bytes = log.into_bytes();
                    if ws_tx.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
            });

            // WebSocket → Docker input
            let exec_id_c = exec_id.clone();
            let docker = state.docker.clone_inner();
            let in_handle = tokio::spawn(async move {
                while let Some(Ok(msg)) = ws_rx.next().await {
                    match msg {
                        Message::Text(ref text) => {
                            // Resize command
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                                if val.get("type").and_then(|t| t.as_str()) == Some("resize") {
                                    let cols = val.get("cols").and_then(|c| c.as_u64()).unwrap_or(80) as u16;
                                    let rows = val.get("rows").and_then(|r| r.as_u64()).unwrap_or(24) as u16;
                                    use bollard::exec::ResizeExecOptions;
                                    let _ = docker.resize_exec(&exec_id_c, ResizeExecOptions { width: cols, height: rows }).await;
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

            tokio::select! {
                _ = out_handle => {},
                _ = in_handle => {},
            }
        }
        StartExecResults::Detached => {
            tracing::error!("Exec started in detached mode");
        }
    }
}

async fn proxy_terminal(
    server_ws: WebSocket,
    agent_url: String,
    agent_token: String,
    container_id: String,
    shell: String,
    user: Option<String>,
) {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::tungstenite;

    // Build agent WebSocket URL
    let agent_ws_url = agent_url.replace("http://", "ws://").replace("https://", "wss://");
    let mut url = format!("{}/api/containers/{}/terminal?token={}&shell={}",
        agent_ws_url, container_id, urlencoding::encode(&agent_token), urlencoding::encode(&shell));
    if let Some(ref u) = user {
        url.push_str(&format!("&user={}", urlencoding::encode(u)));
    }

    // Connect to agent WebSocket
    let agent_conn = match tokio_tungstenite::connect_async(&url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            tracing::error!("Agent WebSocket connection failed: {}", e);
            let (mut tx, _) = server_ws.split();
            let _ = tx.send(Message::Text(format!("\x1b[31mAgent-Verbindung fehlgeschlagen: {}\x1b[0m\r\n", e).into())).await;
            return;
        }
    };

    let (mut agent_tx, mut agent_rx) = agent_conn.split();
    let (mut server_tx, mut server_rx) = server_ws.split();

    // Server WS → Agent WS
    let s2a = tokio::spawn(async move {
        while let Some(Ok(msg)) = server_rx.next().await {
            let agent_msg = match msg {
                Message::Text(t) => tungstenite::Message::Text(t.to_string()),
                Message::Binary(b) => tungstenite::Message::Binary(b.to_vec()),
                Message::Close(_) => break,
                _ => continue,
            };
            if agent_tx.send(agent_msg).await.is_err() { break; }
        }
    });

    // Agent WS → Server WS
    let a2s = tokio::spawn(async move {
        while let Some(Ok(msg)) = agent_rx.next().await {
            let server_msg = match msg {
                tungstenite::Message::Text(t) => Message::Text(t.into()),
                tungstenite::Message::Binary(b) => Message::Binary(b.into()),
                tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            if server_tx.send(server_msg).await.is_err() { break; }
        }
    });

    tokio::select! { _ = s2a => {}, _ = a2s => {} }
}

// === Host Terminal ===

pub async fn host_terminal(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
    Query(query): Query<TerminalQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if !validate_ws_token(&state, &query.token) {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    let env = match state.db.get_environment(&env_id) {
        Some(e) => e,
        None => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };

    if env.is_local {
        ws.on_upgrade(move |socket| handle_host_terminal(socket, state))
    } else {
        let agent_url = env.url.clone();
        let agent_token = env.agent_token.clone().unwrap_or_default();
        ws.on_upgrade(move |socket| proxy_host_terminal(socket, agent_url, agent_token))
    }
}

async fn handle_host_terminal(
    mut socket: WebSocket,
    state: Arc<AppState>,
) {
    use futures_util::{StreamExt, SinkExt};
    use tokio::io::AsyncWriteExt;
    use bollard::exec::StartExecResults;

    // Exec nsenter into our own container (dockpit runs with pid:host + privileged)
    let container_id = "dockpit";
    let cmd = vec!["nsenter", "-t", "1", "-m", "-u", "-i", "-n", "--", "/bin/bash"];
    let exec_id = match state.docker.create_exec(container_id, cmd, None).await {
        Ok(id) => id,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("\x1b[31mHost-Shell fehlgeschlagen: {}\x1b[0m\r\n", e).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    let exec_result = match state.docker.start_exec(&exec_id).await {
        Ok(r) => r,
        Err(e) => {
            let _ = socket.send(Message::Text(format!("\x1b[31mExec fehlgeschlagen: {}\x1b[0m\r\n", e).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    let docker = state.docker.clone_inner();

    match exec_result {
        StartExecResults::Attached { mut output, mut input } => {
            let (mut ws_tx, mut ws_rx) = socket.split();

            let out_handle = tokio::spawn(async move {
                while let Some(Ok(log)) = output.next().await {
                    let bytes = log.into_bytes();
                    if ws_tx.send(Message::Binary(bytes.into())).await.is_err() { break; }
                }
            });

            let exec_id_c = exec_id.clone();
            let docker2 = docker.clone();
            let in_handle = tokio::spawn(async move {
                while let Some(Ok(msg)) = ws_rx.next().await {
                    match msg {
                        Message::Text(ref text) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                                if val.get("type").and_then(|t| t.as_str()) == Some("resize") {
                                    let cols = val.get("cols").and_then(|c| c.as_u64()).unwrap_or(80) as u16;
                                    let rows = val.get("rows").and_then(|r| r.as_u64()).unwrap_or(24) as u16;
                                    use bollard::exec::ResizeExecOptions;
                                    let _ = docker2.resize_exec(&exec_id_c, ResizeExecOptions { width: cols, height: rows }).await;
                                    continue;
                                }
                            }
                            if input.write_all(text.as_bytes()).await.is_err() { break; }
                        }
                        Message::Binary(data) => { if input.write_all(&data).await.is_err() { break; } }
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

async fn proxy_host_terminal(
    server_ws: WebSocket,
    agent_url: String,
    agent_token: String,
) {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::tungstenite;

    let agent_ws_url = agent_url.replace("http://", "ws://").replace("https://", "wss://");
    let url = format!("{}/api/host-terminal?token={}", agent_ws_url, urlencoding::encode(&agent_token));

    let agent_conn = match tokio_tungstenite::connect_async(&url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            tracing::error!("Agent host terminal WebSocket failed: {}", e);
            return;
        }
    };

    let (mut agent_tx, mut agent_rx) = agent_conn.split();
    let (mut srv_tx, mut srv_rx) = server_ws.split();

    // Server -> Agent
    let s2a = tokio::spawn(async move {
        while let Some(Ok(msg)) = srv_rx.next().await {
            let agent_msg = match msg {
                Message::Text(t) => tungstenite::Message::Text(t.to_string()),
                Message::Binary(b) => tungstenite::Message::Binary(b.to_vec()),
                Message::Close(_) => break,
                _ => continue,
            };
            if agent_tx.send(agent_msg).await.is_err() { break; }
        }
    });

    // Agent -> Server
    let a2s = tokio::spawn(async move {
        while let Some(Ok(msg)) = agent_rx.next().await {
            let srv_msg = match msg {
                tungstenite::Message::Text(t) => Message::Text(t.into()),
                tungstenite::Message::Binary(b) => Message::Binary(b.into()),
                tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            if srv_tx.send(srv_msg).await.is_err() { break; }
        }
    });

    tokio::select! { _ = s2a => {}, _ = a2s => {} }
}

// === Live Stats WebSocket ===

pub async fn env_stats_live(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
    Query(query): Query<TerminalQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if !validate_ws_token(&state, &query.token) {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    let env = match state.db.get_environment(&env_id) {
        Some(e) => e,
        None => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };

    if env.is_local {
        ws.on_upgrade(move |socket| handle_stats_local(socket, state))
    } else {
        let agent_url = env.url.clone();
        let agent_token = env.agent_token.clone().unwrap_or_default();
        ws.on_upgrade(move |socket| proxy_stats_to_agent(socket, agent_url, agent_token))
    }
}

async fn handle_stats_local(mut socket: WebSocket, state: Arc<AppState>) {
    use futures_util::SinkExt;

    loop {
        let stats = state.docker.get_all_container_stats().await;
        let snapshot = StatsSnapshot {
            containers: stats,
            timestamp: chrono::Utc::now().timestamp(),
        };
        let json = match serde_json::to_string(&snapshot) {
            Ok(j) => j,
            Err(_) => break,
        };
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn proxy_stats_to_agent(server_ws: WebSocket, agent_url: String, agent_token: String) {
    use futures_util::{StreamExt, SinkExt};
    use tokio_tungstenite::tungstenite;

    let agent_ws_url = agent_url.replace("http://", "ws://").replace("https://", "wss://");
    let url = format!("{}/api/stats?token={}", agent_ws_url, urlencoding::encode(&agent_token));

    let agent_conn = match tokio_tungstenite::connect_async(&url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            tracing::error!("Agent stats WebSocket failed: {}", e);
            let (mut tx, _) = server_ws.split();
            let _ = tx.send(Message::Text(format!("{{\"error\":\"{}\"}}", e).into())).await;
            return;
        }
    };

    let (mut _agent_tx, mut agent_rx) = agent_conn.split();
    let (mut server_tx, mut server_rx) = server_ws.split();

    // Agent → Server (stats data)
    let a2s = tokio::spawn(async move {
        while let Some(Ok(msg)) = agent_rx.next().await {
            let server_msg = match msg {
                tungstenite::Message::Text(t) => Message::Text(t.into()),
                tungstenite::Message::Binary(b) => Message::Binary(b.into()),
                tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            if server_tx.send(server_msg).await.is_err() { break; }
        }
    });

    // Server → Agent (close signal)
    let s2a = tokio::spawn(async move {
        while let Some(Ok(msg)) = server_rx.next().await {
            if matches!(msg, Message::Close(_)) { break; }
        }
    });

    tokio::select! { _ = s2a => {}, _ = a2s => {} }
}

// === Environment-scoped Images ===

pub async fn env_images(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<ImageInfo>>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.list_images().await {
            Ok(i) => Json(ApiResponse::ok(i)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_get(&env, "/api/images").await
    }
}

pub async fn env_prune_images(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.prune_images().await {
            Ok(msg) => Json(ApiResponse::ok(msg)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_post(&env, "/api/images/prune", &()).await
    }
}

pub async fn env_pull_image(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
    Json(req): Json<PullImageRequest>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.pull_image(&req.image).await {
            Ok(_) => Json(ApiResponse::ok(format!("{} gepullt", req.image))),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_post(&env, "/api/images/pull", &req).await
    }
}

pub async fn env_remove_image(
    State(state): State<Arc<AppState>>,
    Path((env_id, image_id)): Path<(String, String)>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    let force = query.get("force").map(|v| v == "true").unwrap_or(false);

    if env.is_local {
        match state.docker.remove_image_force(&image_id, force).await {
            Ok(_) => Json(ApiResponse::ok("Image gelöscht".to_string())),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        let path = format!("/api/images/{}?force={}", urlencoding::encode(&image_id), force);
        agent_del(&env, &path).await
    }
}

// === Environment-scoped Volumes ===

pub async fn env_volumes(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<VolumeInfo>>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.list_volumes().await {
            Ok(v) => Json(ApiResponse::ok(v)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_get(&env, "/api/volumes").await
    }
}

pub async fn env_remove_volume(
    State(state): State<Arc<AppState>>,
    Path((env_id, name)): Path<(String, String)>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    let force = query.get("force").map(|v| v == "true").unwrap_or(false);

    if env.is_local {
        match state.docker.remove_volume(&name, force).await {
            Ok(_) => Json(ApiResponse::ok("Volume gelöscht".to_string())),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_del(&env, &format!("/api/volumes/{}?force={}", urlencoding::encode(&name), force)).await
    }
}

pub async fn env_prune_volumes(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.prune_volumes().await {
            Ok(msg) => Json(ApiResponse::ok(msg)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_post(&env, "/api/volumes/prune", &()).await
    }
}

// === Environment-scoped Networks ===

pub async fn env_networks(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<NetworkInfo>>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        match state.docker.list_networks().await {
            Ok(n) => Json(ApiResponse::ok(n)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_get(&env, "/api/networks").await
    }
}

pub async fn env_remove_network(
    State(state): State<Arc<AppState>>,
    Path((env_id, network_id)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    if env.is_local {
        match state.docker.remove_network(&network_id).await {
            Ok(_) => Json(ApiResponse::ok("Netzwerk gelöscht".to_string())),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_del(&env, &format!("/api/networks/{}", urlencoding::encode(&network_id))).await
    }
}

pub async fn env_prune_networks(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    if env.is_local {
        match state.docker.prune_networks().await {
            Ok(msg) => Json(ApiResponse::ok(msg)),
            Err(e) => Json(ApiResponse::err(e.to_string())),
        }
    } else {
        agent_post(&env, "/api/networks/prune", &()).await
    }
}

// === Environment Management ===

pub async fn list_environments(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<EnvironmentInfo>>> {
    // Use cached environment list if available
    let mut envs = {
        let cache = state.env_cache.read().unwrap();
        cache.clone()
    }.unwrap_or_else(|| {
        let envs = state.db.get_environments();
        *state.env_cache.write().unwrap() = Some(envs.clone());
        envs
    });
    for env in &mut envs {
        env.status = if env.is_local { "online".to_string() } else { "unknown".to_string() };
    }
    Json(ApiResponse::ok(envs))
}

fn invalidate_env_cache(state: &AppState) {
    *state.env_cache.write().unwrap() = None;
}

/// Separate endpoint to check status of a single environment (fast, non-blocking)
pub async fn env_check_status(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<String>> {
    let env = match state.db.get_environment(&env_id) {
        Some(e) => e,
        None => return Json(ApiResponse::err("Nicht gefunden")),
    };
    if env.is_local {
        return Json(ApiResponse::ok("online".to_string()));
    }
    if env.paused {
        return Json(ApiResponse::ok("paused".to_string()));
    }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
    let url = format!("{}/health", env.url);
    let status = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => "online",
        _ => {
            notify_and_email(&state, "connection_error",
                &format!("Server offline: {}", env.name),
                &format!("Cannot reach agent at {}", env.url));
            "offline"
        }
    };
    Json(ApiResponse::ok(status.to_string()))
}

pub async fn create_environment(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateEnvironmentRequest>,
) -> Json<ApiResponse<EnvironmentInfo>> {
    let url = req.url.trim_end_matches('/').to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    // Discover agent
    let health_url = format!("{}/health", url);
    let health_resp = match client.get(&health_url).send().await {
        Ok(r) => r,
        Err(e) => return Json(ApiResponse::err(format!(
            "Agent nicht erreichbar unter {}. Fehler: {}", url, e
        ))),
    };

    let health_data: serde_json::Value = match health_resp.json().await {
        Ok(d) => d,
        Err(_) => return Json(ApiResponse::err("Ungültige Antwort vom Agent")),
    };

    if health_data.get("success") != Some(&serde_json::Value::Bool(true)) {
        return Json(ApiResponse::err("Kein gültiger DockPit Agent"));
    }

    let already_paired = health_data
        .get("data")
        .and_then(|d| d.get("paired"))
        .and_then(|p| p.as_bool())
        .unwrap_or(false);

    if already_paired {
        return Json(ApiResponse::err(
            "Agent bereits gepairt. Agent neustarten zum erneuten Pairen."
        ));
    }

    // Pair
    let token = auth::generate_agent_token();
    let pair_url = format!("{}/api/pair", url);
    let pair_body = serde_json::json!({ "token": token });

    match client.post(&pair_url).json(&pair_body).send().await {
        Ok(resp) => {
            let result: serde_json::Value = resp.json().await.unwrap_or_default();
            if result.get("success") != Some(&serde_json::Value::Bool(true)) {
                let err = result.get("error").and_then(|e| e.as_str()).unwrap_or("Unbekannt");
                return Json(ApiResponse::err(format!("Pairing fehlgeschlagen: {}", err)));
            }
        }
        Err(e) => return Json(ApiResponse::err(format!("Pairing fehlgeschlagen: {}", e))),
    }

    let agent_hostname = health_data
        .get("data")
        .and_then(|d| d.get("hostname"))
        .and_then(|h| h.as_str())
        .unwrap_or("Remote Server");

    let name = if req.name.is_empty() { agent_hostname.to_string() } else { req.name };

    let env = EnvironmentInfo {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        status: "online".to_string(),
        is_local: false,
        agent_token: Some(token),
        paused: false,
    };

    match state.db.create_environment(&env) {
        Ok(_) => {
            invalidate_env_cache(&state);
            state.db.log_audit(&audit_user(&headers), "env_create", Some(&env.name), None);
            Json(ApiResponse::ok(env))
        }
        Err(e) => Json(ApiResponse::err(format!("Speichern fehlgeschlagen: {}", e))),
    }
}

pub async fn discover_agents(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let mut results = Vec::new();
    let existing_urls: Vec<String> = state.db.get_environments().iter().map(|e| e.url.clone()).collect();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();

    // Read ARP table — contains all IPs this host has communicated with.
    // Much faster than scanning entire subnets (dozens of IPs vs 65,000).
    let mut ips_to_check: Vec<String> = Vec::new();
    if let Ok(content) = tokio::fs::read_to_string("/proc/net/arp").await {
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 6 { continue; }
            let ip = fields[0];
            let device = fields[5];
            // Skip Docker interfaces
            if device.starts_with("docker") || device.starts_with("br-") || device.starts_with("veth") { continue; }
            if !existing_urls.iter().any(|u| u.contains(ip)) {
                ips_to_check.push(ip.to_string());
            }
        }
    }

    // Also scan the host's own /24 subnet for agents not yet in ARP table
    if let Ok(output) = tokio::process::Command::new("hostname").arg("-I").output().await {
        let ips = String::from_utf8_lossy(&output.stdout);
        for ip in ips.split_whitespace() {
            let parts: Vec<&str> = ip.split('.').collect();
            if parts.len() == 4 && !ip.starts_with("172.") && !ip.starts_with("127.") && !ip.contains(':') {
                let subnet = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
                for i in 1..=254u8 {
                    let candidate = format!("{}.{}", subnet, i);
                    if !ips_to_check.contains(&candidate) && !existing_urls.iter().any(|u| u.contains(&candidate)) {
                        ips_to_check.push(candidate);
                    }
                }
            }
        }
    }

    ips_to_check.sort();
    ips_to_check.dedup();
    tracing::info!("Scanning {} IPs for agents (ARP + local subnet)", ips_to_check.len());

    // Check all IPs in parallel — usually only a few dozen from ARP + 254 from local subnet
    let mut handles = Vec::new();
    for ip in &ips_to_check {
        let url = format!("http://{}:5522", ip);
        let client = client.clone();
        let handle = tokio::spawn(async move {
            match client.get(format!("{}/health", url)).send().await {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if data.get("success") == Some(&serde_json::Value::Bool(true)) {
                            if let Some(agent_data) = data.get("data") {
                                let mut result = agent_data.clone();
                                if let Some(obj) = result.as_object_mut() {
                                    obj.insert("url".to_string(), serde_json::Value::String(url));
                                }
                                return Some(result);
                            }
                        }
                    }
                    None
                }
                Err(_) => None,
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        if let Ok(Some(agent)) = handle.await {
            results.push(agent);
        }
    }

    Json(ApiResponse::ok(results))
}

pub async fn toggle_env_paused(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let paused = req.get("paused").and_then(|v| v.as_bool()).unwrap_or(false);
    match state.db.toggle_env_paused(&id, paused) {
        Ok(_) => {
            invalidate_env_cache(&state);
            Json(ApiResponse::ok(if paused { "paused" } else { "resumed" }.to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn update_environment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateEnvironmentRequest>,
) -> Json<ApiResponse<String>> {
    let existing = state.db.get_environment(&id);
    if let Some(ref env) = existing {
        if env.is_local {
            return match state.db.update_environment(&id, &req.name, &env.url) {
                Ok(_) => { invalidate_env_cache(&state); Json(ApiResponse::ok("Umgebung aktualisiert".to_string())) }
                Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
            };
        }
    }
    match state.db.update_environment(&id, &req.name, &req.url) {
        Ok(_) => { invalidate_env_cache(&state); Json(ApiResponse::ok("Umgebung aktualisiert".to_string())) }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn delete_environment(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    if let Some(env) = state.db.get_environment(&id) {
        if env.is_local {
            return Json(ApiResponse::err("Lokale Umgebung kann nicht gelöscht werden"));
        }
    }

    match state.db.delete_environment(&id) {
        Ok(_) => {
            invalidate_env_cache(&state);
            state.db.log_audit(&audit_user(&headers), "env_delete", Some(&env_name(&state, &id)), None);
            Json(ApiResponse::ok("Umgebung entfernt".to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

// === Docker Registry Login ===

pub async fn list_registries(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<RegistryInfo>>> {
    let regs = state.db.get_registries()
        .into_iter()
        .map(|(registry, username)| RegistryInfo { registry, username })
        .collect();
    Json(ApiResponse::ok(regs))
}

pub async fn add_registry(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegistryLogin>,
) -> Json<ApiResponse<String>> {
    let registry = if req.registry.trim().is_empty() {
        "docker.io".to_string()
    } else {
        req.registry.trim().to_lowercase()
    };

    // Test login locally first
    let mut child = match tokio::process::Command::new("docker")
        .args(["login", &registry, "-u", &req.username, "--password-stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Json(ApiResponse::err(format!("Docker nicht erreichbar: {}", e))),
    };

    {
        use tokio::io::AsyncWriteExt;
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(req.password.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }
    }

    let result = child.wait_with_output().await;
    match result {
        Ok(o) if o.status.success() => {
            state.db.save_registry(&registry, &req.username, &req.password).ok();

            // Propagate login to all remote agents
            let envs = state.db.get_environments();
            let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap();
            for env in envs {
                if env.is_local { continue; }
                let url = format!("{}/api/docker/login", env.url);
                let body = serde_json::json!({
                    "registry": registry,
                    "username": req.username,
                    "password": req.password,
                });
                let token = env.agent_token.unwrap_or_default();
                let res = client.post(&url)
                    .header("X-Agent-Token", &token)
                    .json(&body)
                    .send().await;
                match res {
                    Ok(r) if r.status().is_success() => tracing::info!("Registry login propagated to {}", env.name),
                    _ => tracing::warn!("Failed to propagate registry login to {}", env.name),
                }
            }

            Json(ApiResponse::ok(format!("Login erfolgreich: {} (an alle Server verteilt)", registry)))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            Json(ApiResponse::err(format!("Login fehlgeschlagen: {}", err.trim())))
        }
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

pub async fn remove_registry(
    State(state): State<Arc<AppState>>,
    Path(registry): Path<String>,
) -> Json<ApiResponse<String>> {
    // Docker logout locally
    let _ = tokio::process::Command::new("docker")
        .args(["logout", &registry])
        .output()
        .await;

    // Propagate logout to all agents
    let envs = state.db.get_environments();
    let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
    for env in envs {
        if env.is_local { continue; }
        let url = format!("{}/api/docker/logout/{}", env.url, urlencoding::encode(&registry));
        let token = env.agent_token.unwrap_or_default();
        let _ = client.delete(&url).header("X-Agent-Token", &token).send().await;
    }

    match state.db.delete_registry(&registry) {
        Ok(_) => Json(ApiResponse::ok("Registry entfernt (von allen Servern)".to_string())),
        Err(e) => Json(ApiResponse::err(format!("Fehler: {}", e))),
    }
}

// === Docker Hub Search ===

pub async fn search_docker_hub(
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let q = query.get("q").map(|s| s.as_str()).unwrap_or("");
    if q.len() < 2 {
        return Json(ApiResponse::ok(vec![]));
    }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
    let url = format!("https://hub.docker.com/v2/search/repositories/?query={}&page_size=8", urlencoding::encode(q));
    match client.get(&url).send().await {
        Ok(resp) => {
            let data: serde_json::Value = resp.json().await.unwrap_or_default();
            let results = data["results"].as_array().map(|arr| {
                arr.iter().map(|r| serde_json::json!({
                    "name": r["repo_name"].as_str().unwrap_or(""),
                    "description": r["short_description"].as_str().unwrap_or("").chars().take(80).collect::<String>(),
                    "is_official": r["is_official"].as_bool().unwrap_or(false),
                    "star_count": r["star_count"].as_i64().unwrap_or(0),
                })).collect()
            }).unwrap_or_default();
            Json(ApiResponse::ok(results))
        }
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

// === Health Checks ===

pub async fn env_health_checks(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<ContainerHealth>>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    if env.is_local {
        Json(ApiResponse::ok(state.docker.get_container_health().await))
    } else {
        agent_get(&env, "/api/health").await
    }
}

// === Stack Templates ===

pub async fn list_templates(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<StackTemplate>>> {
    Json(ApiResponse::ok(state.db.get_templates()))
}

pub async fn get_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<StackTemplate>> {
    match state.db.get_template(&id) {
        Some(t) => Json(ApiResponse::ok(t)),
        None => Json(ApiResponse::err("Template not found")),
    }
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateTemplateRequest>,
) -> Json<ApiResponse<StackTemplate>> {
    let t = StackTemplate {
        id: format!("tpl-custom-{}", uuid::Uuid::new_v4()),
        name: req.name,
        description: req.description,
        category: req.category.unwrap_or_else(|| "custom".into()),
        compose_content: req.compose_content,
        env_content: req.env_content,
        icon: req.icon,
        is_default: false,
        created_at: None,
    };
    state.db.log_audit(&audit_user(&headers), "template_create", Some(&t.name), None);
    match state.db.create_template(&t) {
        Ok(_) => Json(ApiResponse::ok(t)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn update_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Json<ApiResponse<String>> {
    let name = req.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let description = req.get("description").and_then(|v| v.as_str());
    let compose_content = req.get("compose_content").and_then(|v| v.as_str()).unwrap_or("");
    let env_content = req.get("env_content").and_then(|v| v.as_str());
    let icon = req.get("icon").and_then(|v| v.as_str());
    if name.is_empty() || compose_content.is_empty() {
        return Json(ApiResponse::err("Name and compose content required"));
    }
    match state.db.update_template(&id, name, description, compose_content, env_content) {
        Ok(_) => {
            if let Some(ico) = icon {
                state.db.update_template_icon(&id, ico).ok();
            }
            Json(ApiResponse::ok("Template updated".into()))
        }
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    state.db.log_audit(&audit_user(&headers), "template_delete", Some(&id), None);
    match state.db.delete_template(&id) {
        Ok(_) => Json(ApiResponse::ok("Deleted".into())),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

// === Audit Log ===

pub async fn get_audit_log(
    State(state): State<Arc<AppState>>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<AuditResponse>> {
    let limit = query.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50i64);
    let offset = query.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0i64);
    let user = query.get("user").map(|s| s.as_str());
    let action = query.get("action").map(|s| s.as_str());
    let entries = state.db.get_audit_log(limit, offset, user, action);
    let total = state.db.get_audit_count(user, action);
    Json(ApiResponse::ok(AuditResponse { entries, total }))
}

// === Vulnerability Scanning ===

pub async fn env_get_vulnerabilities(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<VulnerabilityScan>>> {
    Json(ApiResponse::ok(state.db.get_scan_results(&env_id)))
}

pub async fn env_get_scan_history(
    State(state): State<Arc<AppState>>,
    Path((env_id, image)): Path<(String, String)>,
) -> Json<ApiResponse<Vec<VulnerabilityScan>>> {
    let decoded = urlencoding::decode(&image).unwrap_or_default().to_string();
    Json(ApiResponse::ok(state.db.get_scan_history(&env_id, &decoded)))
}

pub async fn env_vuln_scan_status(
    State(state): State<Arc<AppState>>,
    Path(_env_id): Path<String>,
) -> Json<ApiResponse<VulnScanStatus>> {
    use std::sync::atomic::Ordering;
    Json(ApiResponse::ok(VulnScanStatus {
        running: state.vuln_scan_running.load(Ordering::SeqCst),
        total: state.vuln_scan_total.load(Ordering::SeqCst),
        done: state.vuln_scan_done.load(Ordering::SeqCst),
    }))
}

pub async fn env_scan_vulnerabilities(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<String>> {
    use std::sync::atomic::Ordering;

    if state.vuln_scan_running.load(Ordering::SeqCst) {
        return Json(ApiResponse { success: false, data: None, error: Some("Scan already running".to_string()) });
    }

    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    state.db.log_audit(&audit_user(&headers), "vuln_scan", Some(&env_name(&state, &env_id)), None);
    state.vuln_scan_running.store(true, Ordering::SeqCst);
    state.vuln_scan_total.store(0, Ordering::SeqCst);
    state.vuln_scan_done.store(0, Ordering::SeqCst);

    let state_clone = state.clone();
    let env_id_clone = env_id.clone();
    tokio::spawn(async move {
        let containers: Vec<ContainerInfo> = if env.is_local {
            state_clone.docker.list_containers().await.unwrap_or_default()
        } else {
            let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
            let url = format!("{}/api/containers", env.url);
            match client.get(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
                Ok(resp) => resp.json::<ApiResponse<Vec<ContainerInfo>>>().await
                    .ok().and_then(|r| r.data).unwrap_or_default(),
                Err(_) => vec![],
            }
        };

        // Propagate Docker login to agent before scanning (scout needs it)
        if !env.is_local {
            let creds_list = state_clone.db.get_all_registry_credentials();
            let login_client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
            for (registry, username, password) in &creds_list {
                let url = format!("{}/api/docker/login", env.url);
                let body = serde_json::json!({ "registry": registry, "username": username, "password": password });
                let _ = login_client.post(&url)
                    .header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
                    .json(&body).send().await;
            }
        }

        // Unique images only
        let unique_images: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            containers.iter().filter(|c| seen.insert(c.image.clone())).map(|c| c.image.clone()).collect()
        };
        state_clone.vuln_scan_total.store(unique_images.len(), Ordering::SeqCst);
        state_clone.vuln_scan_done.store(0, Ordering::SeqCst);

        for image in &unique_images {
            let result = if env.is_local {
                scout_scan_image(image).await
            } else {
                let client = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().unwrap();
                let url = format!("{}/api/vulnerabilities/scan", env.url);
                let body = serde_json::json!({ "image": image });
                match client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
                    .json(&body).send().await
                {
                    Ok(resp) => {
                        match resp.json::<ApiResponse<serde_json::Value>>().await {
                            Ok(r) if r.success => {
                                if let Some(data) = r.data {
                                    Ok(VulnerabilityScan {
                                        id: None,
                                        env_id: String::new(),
                                        image: data["image"].as_str().unwrap_or(image).to_string(),
                                        critical: data["critical"].as_i64().unwrap_or(0) as i32,
                                        high: data["high"].as_i64().unwrap_or(0) as i32,
                                        medium: data["medium"].as_i64().unwrap_or(0) as i32,
                                        low: data["low"].as_i64().unwrap_or(0) as i32,
                                        total: data["total"].as_i64().unwrap_or(0) as i32,
                                        cves_json: data["cves_json"].as_str().map(|s| s.to_string()),
                                        scanned_at: None,
                                    })
                                } else { Err("No data from agent".to_string()) }
                            }
                            Ok(r) => Err(r.error.unwrap_or_else(|| "Agent scan failed".to_string())),
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    Err(e) => Err(e.to_string()),
                }
            };

            match result {
                Ok(mut scan) => {
                    scan.env_id = env_id_clone.clone();
                    tracing::info!("Vuln scan OK: {} — C:{} H:{} M:{} L:{}", scan.image, scan.critical, scan.high, scan.medium, scan.low);
                    state_clone.db.save_scan_result(&scan).ok();
                }
                Err(e) => {
                    tracing::warn!("Vuln scan FAILED for {}: {}", image, e);
                }
            }
            state_clone.vuln_scan_done.fetch_add(1, Ordering::SeqCst);
        }

        state_clone.vuln_scan_running.store(false, Ordering::SeqCst);
        notify_and_email(&state_clone,
            "scan_complete",
            "Vulnerability scan complete",
            &format!("{} images scanned", unique_images.len()),
        );
    });

    Json(ApiResponse::ok("Scan started".into()))
}

pub async fn env_scan_single_image(
    State(state): State<Arc<AppState>>,
    Path((env_id, image)): Path<(String, String)>,
) -> Json<ApiResponse<VulnerabilityScan>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };
    let decoded_image = urlencoding::decode(&image).unwrap_or_default().to_string();

    let result = if env.is_local {
        scout_scan_image(&decoded_image).await
    } else {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().unwrap();
        let url = format!("{}/api/vulnerabilities/scan", env.url);
        let body = serde_json::json!({ "image": decoded_image });
        match client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
            .json(&body).send().await
        {
            Ok(resp) => {
                match resp.json::<ApiResponse<serde_json::Value>>().await {
                    Ok(r) if r.success => {
                        if let Some(data) = r.data {
                            Ok(VulnerabilityScan {
                                id: None, env_id: String::new(),
                                image: data["image"].as_str().unwrap_or(&decoded_image).to_string(),
                                critical: data["critical"].as_i64().unwrap_or(0) as i32,
                                high: data["high"].as_i64().unwrap_or(0) as i32,
                                medium: data["medium"].as_i64().unwrap_or(0) as i32,
                                low: data["low"].as_i64().unwrap_or(0) as i32,
                                total: data["total"].as_i64().unwrap_or(0) as i32,
                                cves_json: data["cves_json"].as_str().map(|s| s.to_string()),
                                scanned_at: None,
                            })
                        } else { Err("No data from agent".to_string()) }
                    }
                    Ok(r) => Err(r.error.unwrap_or_else(|| "Agent scan failed".to_string())),
                    Err(e) => Err(e.to_string()),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    };

    match result {
        Ok(mut scan) => {
            scan.env_id = env_id;
            state.db.save_scan_result(&scan).ok();
            Json(ApiResponse::ok(scan))
        }
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// Execute trivy image scan (SARIF format)
async fn scout_scan_image(image: &str) -> Result<VulnerabilityScan, String> {
    let output = tokio::process::Command::new("trivy")
        .args(["image", "--format", "sarif", "--quiet", image])
        .output()
        .await
        .map_err(|e| format!("trivy not available: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Scan failed: {}", stderr.chars().take(200).collect::<String>()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sarif: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|_| "Failed to parse trivy SARIF output".to_string())?;

    let mut critical = 0i32;
    let mut high = 0i32;
    let mut medium = 0i32;
    let mut low = 0i32;
    let mut cves = Vec::new();

    if let Some(runs) = sarif.get("runs").and_then(|r| r.as_array()) {
        if let Some(run) = runs.first() {
            let rules: std::collections::HashMap<String, &serde_json::Value> = run
                .pointer("/tool/driver/rules")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|r| r.get("id").and_then(|id| id.as_str()).map(|id| (id.to_string(), r))).collect())
                .unwrap_or_default();

            if let Some(results) = run.get("results").and_then(|r| r.as_array()) {
                for result in results {
                    let rule_id = result.get("ruleId").and_then(|r| r.as_str()).unwrap_or("");
                    let rule = rules.get(rule_id);

                    // Trivy uses level in result or properties/precision in rule
                    let severity = result.get("level").and_then(|l| l.as_str())
                        .map(|l| match l { "error" => "CRITICAL", "warning" => "HIGH", "note" => "MEDIUM", _ => "LOW" })
                        .or_else(|| rule.and_then(|r| r.pointer("/properties/cvssV3_severity")).and_then(|s| s.as_str()))
                        .or_else(|| rule.and_then(|r| r.pointer("/properties/security-severity")).and_then(|s| s.as_str()).map(|s| {
                            let score: f64 = s.parse().unwrap_or(0.0);
                            if score >= 9.0 { "CRITICAL" } else if score >= 7.0 { "HIGH" } else if score >= 4.0 { "MEDIUM" } else { "LOW" }
                        }))
                        .unwrap_or("LOW");

                    match severity.to_uppercase().as_str() {
                        "CRITICAL" => critical += 1,
                        "HIGH" => high += 1,
                        "MEDIUM" => medium += 1,
                        "LOW" => low += 1,
                        _ => low += 1,
                    }

                    let package = result.pointer("/locations/0/logicalLocations/0/fullyQualifiedName")
                        .and_then(|s| s.as_str()).unwrap_or("");
                    let description = rule
                        .and_then(|r| r.get("shortDescription").and_then(|d| d.get("text")).and_then(|t| t.as_str()))
                        .or_else(|| rule.and_then(|r| r.get("helpUri").and_then(|u| u.as_str())))
                        .unwrap_or("");

                    cves.push(serde_json::json!({
                        "id": rule_id,
                        "severity": severity,
                        "package": package,
                        "version": "",
                        "fixed": "",
                        "description": description.chars().take(200).collect::<String>(),
                    }));
                }
            }
        }
    }

    let total = critical + high + medium + low;

    Ok(VulnerabilityScan {
        id: None,
        env_id: String::new(),
        image: image.to_string(),
        critical, high, medium, low, total,
        cves_json: Some(serde_json::to_string(&cves).unwrap_or_default()),
        scanned_at: None,
    })
}

// === Container Events ===

pub async fn env_get_events(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<EventsResponse>> {
    let limit = query.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50i64);
    let offset = query.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0i64);
    let events = state.db.get_events(&env_id, limit, offset);
    let total = state.db.get_events_count(&env_id);
    Json(ApiResponse::ok(EventsResponse { events, total }))
}

pub async fn env_refresh_events(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<String>> {
    let env = match get_env(&state, &env_id) {
        Ok(e) => e,
        Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
    };

    let events = if env.is_local {
        let mut evts = state.docker.get_recent_events(3600).await;
        for e in &mut evts { e.env_id = env_id.clone(); }
        evts
    } else {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
        let url = format!("{}/api/events?since=3600", env.url);
        match client.get(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
            Ok(resp) => {
                let mut evts: Vec<ContainerEvent> = resp.json::<ApiResponse<Vec<ContainerEvent>>>().await
                    .ok().and_then(|r| r.data).unwrap_or_default();
                for e in &mut evts { e.env_id = env_id.clone(); }
                evts
            }
            Err(_) => vec![],
        }
    };

    let count = events.len();
    state.db.save_events(&events);
    Json(ApiResponse::ok(format!("{} events collected", count)))
}

/// Background: collect events from all environments for a given time window
pub async fn collect_events_since(state: Arc<AppState>, since_secs: i64) {
    let envs = state.db.get_environments();
    for env in &envs {
        if env.paused { continue; } // Skip paused environments
        let events = if env.is_local {
            let mut evts = state.docker.get_recent_events(since_secs).await;
            for e in &mut evts { e.env_id = env.id.clone(); }
            evts
        } else {
            let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap();
            let url = format!("{}/api/events?since={}", env.url, since_secs);
            match client.get(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
                Ok(resp) => {
                    let mut evts: Vec<ContainerEvent> = resp.json::<ApiResponse<Vec<ContainerEvent>>>().await
                        .ok().and_then(|r| r.data).unwrap_or_default();
                    for e in &mut evts { e.env_id = env.id.clone(); }
                    evts
                }
                Err(_) => vec![],
            }
        };
        state.db.save_events(&events);
    }
    state.db.cleanup_old_events();
}

// === Notifications ===

pub async fn get_notifications(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<NotificationInfo>>> {
    Json(ApiResponse::ok(state.db.get_notifications(50)))
}

pub async fn get_unread_count(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<UnreadCount>> {
    Json(ApiResponse::ok(UnreadCount { count: state.db.get_unread_count() }))
}

pub async fn mark_notification_read(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<String>> {
    match state.db.mark_notification_read(id) {
        Ok(_) => Json(ApiResponse::ok("OK".into())),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn mark_all_notifications_read(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<String>> {
    match state.db.mark_all_notifications_read() {
        Ok(_) => Json(ApiResponse::ok("OK".into())),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn delete_notification(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<String>> {
    match state.db.delete_notification(id) {
        Ok(_) => Json(ApiResponse::ok("OK".into())),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

// === Scheduled Jobs ===

pub async fn list_scheduled_jobs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<Vec<ScheduledJob>>> {
    let env_id = query.get("env_id").map(|s| s.as_str());
    Json(ApiResponse::ok(state.db.get_scheduled_jobs(env_id)))
}

pub async fn create_scheduled_job(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateJobRequest>,
) -> Json<ApiResponse<ScheduledJob>> {
    let job = ScheduledJob {
        id: uuid::Uuid::new_v4().to_string(),
        env_id: req.env_id,
        job_type: req.job_type,
        enabled: true,
        interval_hours: req.interval_hours,
        stack_name: req.stack_name,
        last_run: None,
        next_run: Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        last_result: None,
        last_message: None,
    };
    match state.db.create_scheduled_job(&job) {
        Ok(_) => {
            state.db.log_audit(&audit_user(&headers), "job_create", Some(&job.job_type), None);
            Json(ApiResponse::ok(job))
        }
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn update_scheduled_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateJobRequest>,
) -> Json<ApiResponse<String>> {
    match state.db.update_scheduled_job(&id, req.enabled, req.interval_hours) {
        Ok(_) => Json(ApiResponse::ok("Updated".into())),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn delete_scheduled_job(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.db.delete_scheduled_job(&id) {
        Ok(_) => {
            state.db.log_audit(&audit_user(&headers), "job_delete", Some(&id), None);
            Json(ApiResponse::ok("Deleted".into()))
        }
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

pub async fn run_scheduled_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    let jobs = state.db.get_scheduled_jobs(None);
    let job = match jobs.iter().find(|j| j.id == id) {
        Some(j) => j.clone(),
        None => return Json(ApiResponse::err("Job not found")),
    };
    tokio::spawn(execute_job(state, job));
    Json(ApiResponse::ok("Job started".into()))
}

pub async fn execute_job(state: Arc<AppState>, job: ScheduledJob) {
    let env = match state.db.get_environment(&job.env_id) {
        Some(e) => e,
        None => {
            state.db.update_job_result(&job.id, "error", "Environment not found", "").ok();
            return;
        }
    };

    let (result, message) = match job.job_type.as_str() {
        "update_check" => {
            // Reuse existing update check logic for this environment
            let containers: Vec<crate::models::ContainerInfo> = if env.is_local {
                state.docker.list_containers().await.unwrap_or_default()
            } else {
                let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
                let url = format!("{}/api/containers", env.url);
                match client.get(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).send().await {
                    Ok(resp) => resp.json::<ApiResponse<Vec<crate::models::ContainerInfo>>>().await
                        .ok().and_then(|r| r.data).unwrap_or_default(),
                    Err(_) => vec![],
                }
            };

            let mut checked = 0;
            let mut outdated = 0;
            let creds: Vec<_> = state.db.get_all_registry_credentials()
                .into_iter()
                .map(|(r, u, p)| serde_json::json!({"registry": r, "username": u, "password": p}))
                .collect();

            for c in &containers {
                if c.state != "running" { continue; }
                checked += 1;
                let check_result = if env.is_local {
                    state.docker.check_image_update(&c.image).await.ok()
                } else {
                    let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().unwrap();
                    let url = format!("{}/api/containers/{}/check-update", env.url, c.id);
                    let body = serde_json::json!({ "credentials": creds });
                    client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or(""))
                        .json(&body).send().await.ok()
                        .and_then(|r| futures_lite::future::block_on(r.json::<ApiResponse<crate::models::ImageUpdateCheck>>()).ok())
                        .and_then(|r| r.data)
                        .map(|d| (d.outdated, d.current_id, d.latest_id))
                };
                if let Some((is_outdated, current, latest)) = check_result {
                    if is_outdated { outdated += 1; }
                    state.db.save_update_check(&c.name, &c.image, &env.name, &env.id, is_outdated, Some(&current), Some(&latest)).ok();
                }
            }
            ("success".to_string(), format!("{} checked, {} outdated", checked, outdated))
        }
        "system_prune" => {
            let mut actions = 0;
            if env.is_local {
                if state.docker.prune_images().await.is_ok() { actions += 1; }
                if state.docker.prune_volumes().await.is_ok() { actions += 1; }
                if state.docker.prune_networks().await.is_ok() { actions += 1; }
            } else {
                let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build().unwrap();
                let token = env.agent_token.as_deref().unwrap_or("");
                for path in &["/api/images/prune", "/api/volumes/prune", "/api/networks/prune"] {
                    let url = format!("{}{}", env.url, path);
                    if client.post(&url).header("X-Agent-Token", token).json(&()).send().await.is_ok() {
                        actions += 1;
                    }
                }
            }
            ("success".to_string(), format!("{} prune actions completed", actions))
        }
        "stack_redeploy" => {
            let stack_name = job.stack_name.as_deref().unwrap_or("");
            if stack_name.is_empty() {
                ("error".to_string(), "No stack name specified".to_string())
            } else if env.is_local {
                match state.stacks.redeploy_stack(stack_name).await {
                    Ok(o) => ("success".to_string(), o),
                    Err(e) => ("error".to_string(), e),
                }
            } else {
                let client = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().unwrap();
                let url = format!("{}/api/stacks/{}/redeploy", env.url, stack_name);
                match client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).json(&()).send().await {
                    Ok(resp) => match resp.json::<ApiResponse<String>>().await {
                        Ok(r) if r.success => ("success".to_string(), r.data.unwrap_or_default()),
                        Ok(r) => ("error".to_string(), r.error.unwrap_or_default()),
                        Err(e) => ("error".to_string(), e.to_string()),
                    },
                    Err(e) => ("error".to_string(), e.to_string()),
                }
            }
        }
        _ => ("error".to_string(), format!("Unknown job type: {}", job.job_type)),
    };

    let next_run = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(job.interval_hours as i64))
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default();

    state.db.update_job_result(&job.id, &result, &message, &next_run).ok();

    // Create notification
    let env_name = state.db.get_environment(&job.env_id).map(|e| e.name).unwrap_or_default();
    let title = format!("{}: {}", job.job_type.replace('_', " "), env_name);
    let ntype = if result == "success" { "job_success" } else { "job_error" };
    notify_and_email(&state, ntype, &title, &message);

    tracing::info!("Scheduled job {} ({}) completed: {} - {}", job.id, job.job_type, result, message);
}

/// Called every 60 seconds from the scheduler loop in main.rs
pub async fn run_due_jobs(state: Arc<AppState>) {
    let due_jobs = state.db.get_due_jobs();
    for job in due_jobs {
        tracing::info!("Executing scheduled job: {} ({})", job.job_type, job.env_id);
        execute_job(state.clone(), job).await;
    }
}

// === Stacks (local + remote via agent) ===

macro_rules! env_or_err {
    ($state:expr, $env_id:expr) => {
        match get_env($state, $env_id) {
            Ok(e) => e,
            Err(e) => return Json(ApiResponse { success: false, data: None, error: e.0.error }),
        }
    };
}

pub async fn env_list_stacks(
    State(state): State<Arc<AppState>>,
    Path(env_id): Path<String>,
) -> Json<ApiResponse<Vec<StackInfo>>> {
    let env = env_or_err!(&state, &env_id);
    if env.is_local {
        match state.stacks.list_stacks().await { Ok(s) => Json(ApiResponse::ok(s)), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_get(&env, "/api/stacks").await }
}

pub async fn env_get_stack(
    State(state): State<Arc<AppState>>,
    Path((env_id, name)): Path<(String, String)>,
) -> Json<ApiResponse<StackDetail>> {
    let env = env_or_err!(&state, &env_id);
    if env.is_local {
        match state.stacks.get_stack_detail(&name).await { Ok(d) => Json(ApiResponse::ok(d)), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_get(&env, &format!("/api/stacks/{}", name)).await }
}

fn validate_compose_yaml(content: &str) -> Result<(), String> {
    // Reject YAML anchors/aliases (potential billion laughs attack)
    if content.contains("<<:") || content.contains("*") && content.contains("&") {
        // More precise check: look for YAML anchor definitions
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("&") && !trimmed.starts_with('#') {
                // Check if it looks like a YAML anchor (not a shell command)
                let before_amp = trimmed.split('&').next().unwrap_or("");
                if before_amp.trim().ends_with(':') || before_amp.trim().is_empty() || trimmed.starts_with("- &") {
                    return Err("YAML anchors/aliases are not allowed".to_string());
                }
            }
        }
    }

    // Parse YAML
    let doc: serde_yaml::Value = serde_yaml::from_str(content)
        .map_err(|e| format!("Invalid YAML: {}", e))?;

    // Must be a mapping at top level
    let map = doc.as_mapping().ok_or("YAML must be a mapping at top level")?;

    // Whitelist top-level keys
    let allowed_keys = ["services", "volumes", "networks", "configs", "secrets", "version", "name", "x-"];
    for key in map.keys() {
        if let Some(k) = key.as_str() {
            if !allowed_keys.iter().any(|a| k == *a || k.starts_with("x-")) {
                return Err(format!("Unknown top-level key: '{}'. Allowed: services, volumes, networks, configs, secrets, name", k));
            }
        }
    }

    // Must have 'services' key
    if !map.contains_key(&serde_yaml::Value::String("services".to_string())) {
        return Err("Missing required 'services' key".to_string());
    }

    Ok(())
}

pub async fn env_create_stack(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(env_id): Path<String>,
    Json(req): Json<CreateStackRequest>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    // Validate compose YAML
    if let Err(e) = validate_compose_yaml(&req.compose_content) {
        return Json(ApiResponse::err(e));
    }
    state.db.log_audit(&audit_user(&headers), "stack_create", Some(&req.name), Some(&format!("Server: {}", env_name(&state, &env_id))));
    if env.is_local {
        match state.stacks.create_stack(&req) { Ok(_) => Json(ApiResponse::ok(format!("Stack '{}' erstellt", req.name))), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_post(&env, "/api/stacks", &req).await }
}

pub async fn env_update_stack(
    State(state): State<Arc<AppState>>,
    Path((env_id, name)): Path<(String, String)>,
    Json(req): Json<UpdateStackRequest>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    if let Err(e) = validate_compose_yaml(&req.compose_content) {
        return Json(ApiResponse::err(e));
    }
    if env.is_local {
        match state.stacks.update_stack(&name, &req) { Ok(_) => Json(ApiResponse::ok("Aktualisiert".into())), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_put(&env, &format!("/api/stacks/{}", name), &req).await }
}

pub async fn env_delete_stack(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, name)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    state.db.log_audit(&audit_user(&headers), "stack_delete", Some(&name), Some(&format!("Server: {}", env_name(&state, &env_id))));
    if env.is_local {
        match state.stacks.delete_stack(&name) { Ok(_) => Json(ApiResponse::ok("Gelöscht".into())), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_del(&env, &format!("/api/stacks/{}", name)).await }
}

pub async fn env_deploy_stack(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, name)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    state.db.log_audit(&audit_user(&headers), "stack_deploy", Some(&name), Some(&format!("Server: {}", env_name(&state, &env_id))));
    if env.is_local {
        match state.stacks.deploy_stack(&name).await { Ok(o) => Json(ApiResponse::ok(o)), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_post(&env, &format!("/api/stacks/{}/deploy", name), &()).await }
}

pub async fn env_stop_stack(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path((env_id, name)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    state.db.log_audit(&audit_user(&headers), "stack_stop", Some(&name), Some(&format!("Server: {}", env_name(&state, &env_id))));
    if env.is_local {
        match state.stacks.stop_stack(&name).await { Ok(o) => Json(ApiResponse::ok(o)), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_post(&env, &format!("/api/stacks/{}/stop", name), &()).await }
}

pub async fn env_restart_stack(
    State(state): State<Arc<AppState>>,
    Path((env_id, name)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    if env.is_local {
        match state.stacks.restart_stack(&name).await { Ok(o) => Json(ApiResponse::ok(o)), Err(e) => Json(ApiResponse::err(e)) }
    } else { agent_post(&env, &format!("/api/stacks/{}/restart", name), &()).await }
}

pub async fn env_redeploy_stack(
    State(state): State<Arc<AppState>>,
    Path((env_id, name)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    let env = env_or_err!(&state, &env_id);
    if env.is_local {
        match state.stacks.redeploy_stack(&name).await { Ok(o) => Json(ApiResponse::ok(o)), Err(e) => Json(ApiResponse::err(e)) }
    } else {
        // Longer timeout for pull + recreate
        let client = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().unwrap();
        let url = format!("{}/api/stacks/{}/redeploy", env.url, name);
        match client.post(&url).header("X-Agent-Token", env.agent_token.as_deref().unwrap_or("")).json(&()).send().await {
            Ok(resp) => match resp.json::<ApiResponse<String>>().await { Ok(d) => Json(d), Err(e) => Json(ApiResponse::err(e.to_string())) },
            Err(e) => Json(ApiResponse::err(format!("Agent: {}", e))),
        }
    }
}

// === Backup & Restore ===

fn get_backup_dir(state: &AppState) -> String {
    state.db.get_setting("backup_dir").unwrap_or_else(|| "/data/backups".to_string())
}

fn now_in_configured_tz(state: &AppState) -> chrono::DateTime<chrono_tz::Tz> {
    let tz_name = state.db.get_setting("timezone").unwrap_or_else(|| "UTC".to_string());
    let tz: chrono_tz::Tz = tz_name.parse().unwrap_or(chrono_tz::UTC);
    chrono::Utc::now().with_timezone(&tz)
}

fn enforce_retention(dir: &str, max_count: usize) {
    let mut files: Vec<_> = std::fs::read_dir(dir).ok()
        .map(|entries| entries.filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("dockpit_") && n.ends_with(".db")
            }).collect())
        .unwrap_or_default();
    files.sort_by_key(|e| e.file_name());
    while files.len() > max_count {
        if let Some(oldest) = files.first() { std::fs::remove_file(oldest.path()).ok(); }
        files.remove(0);
    }
}

pub async fn create_backup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<ApiResponse<BackupInfo>> {
    let dir = get_backup_dir(&state);
    let now = now_in_configured_tz(&state);
    let filename = format!("dockpit_{}.db", now.format("%Y-%m-%d_%H-%M-%S"));
    let full_path = format!("{}/{}", dir, filename);

    match state.db.backup_to(&full_path) {
        Ok(_) => {
            let size = std::fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);
            let retention: usize = state.db.get_setting("backup_retention")
                .and_then(|v| v.parse().ok()).unwrap_or(7);
            enforce_retention(&dir, retention);
            notify_and_email(&state, "backup_success", "Backup created", &filename);
            state.db.log_audit(&audit_user(&headers), "backup_create", Some(&filename), None);
            Json(ApiResponse::ok(BackupInfo {
                filename, size_bytes: size, created_at: now.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
            }))
        }
        Err(e) => Json(ApiResponse::err(format!("Backup failed: {}", e))),
    }
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<BackupInfo>>> {
    let dir = get_backup_dir(&state);
    let now = now_in_configured_tz(&state);
    let offset = now.format("%:z").to_string();

    let mut backups: Vec<BackupInfo> = std::fs::read_dir(&dir).ok()
        .map(|entries| entries.filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("dockpit_") && n.ends_with(".db")
            })
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let ts = name.strip_prefix("dockpit_").and_then(|s| s.strip_suffix(".db")).unwrap_or("");
                let formatted = if ts.len() >= 19 {
                    format!("{}T{}{}", &ts[..10], ts[11..].replace('-', ":"), offset)
                } else { ts.to_string() };
                BackupInfo { filename: name, size_bytes: size, created_at: formatted }
            }).collect())
        .unwrap_or_default();
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Json(ApiResponse::ok(backups))
}

pub async fn download_backup(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> Response {
    if filename.contains("..") || filename.contains('/') || !filename.starts_with("dockpit_") {
        return (StatusCode::BAD_REQUEST, "Invalid filename").into_response();
    }
    let path = format!("{}/{}", get_backup_dir(&state), filename);
    match std::fs::read(&path) {
        Ok(data) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
            ],
            data,
        ).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Backup not found").into_response(),
    }
}

pub async fn delete_backup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(filename): Path<String>,
) -> Json<ApiResponse<String>> {
    if filename.contains("..") || filename.contains('/') || !filename.starts_with("dockpit_") {
        return Json(ApiResponse::err("Invalid filename"));
    }
    let path = format!("{}/{}", get_backup_dir(&state), filename);
    match std::fs::remove_file(&path) {
        Ok(_) => {
            state.db.log_audit(&audit_user(&headers), "backup_delete", Some(&filename), None);
            Json(ApiResponse::ok("Deleted".to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Delete failed: {}", e))),
    }
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(filename): Path<String>,
) -> Json<ApiResponse<String>> {
    if filename.contains("..") || filename.contains('/') || !filename.starts_with("dockpit_") {
        return Json(ApiResponse::err("Invalid filename"));
    }
    let path = format!("{}/{}", get_backup_dir(&state), filename);
    if !std::path::Path::new(&path).exists() {
        return Json(ApiResponse::err("Backup not found"));
    }
    state.db.log_audit(&audit_user(&headers), "backup_restore", Some(&filename), None);
    match state.db.restore_from(&path) {
        Ok(_) => {
            tracing::info!("Database restored from {}", filename);
            Json(ApiResponse::ok("Restored".to_string()))
        }
        Err(e) => Json(ApiResponse::err(format!("Restore failed: {}", e))),
    }
}

pub async fn upload_restore(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> Json<ApiResponse<String>> {
    let dir = get_backup_dir(&state);
    std::fs::create_dir_all(&dir).ok();

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let data = match field.bytes().await {
                Ok(d) => d,
                Err(e) => return Json(ApiResponse::err(format!("Upload failed: {}", e))),
            };
            if data.len() < 16 || &data[..16] != b"SQLite format 3\0" {
                return Json(ApiResponse::err("Invalid file: not a SQLite database"));
            }
            let temp_path = format!("{}/upload_restore_temp.db", dir);
            if let Err(e) = std::fs::write(&temp_path, &data) {
                return Json(ApiResponse::err(format!("Write failed: {}", e)));
            }
            state.db.log_audit(&audit_user(&headers), "backup_restore_upload", None, None);
            match state.db.restore_from(&temp_path) {
                Ok(_) => {
                    std::fs::remove_file(&temp_path).ok();
                    tracing::info!("Database restored from uploaded file");
                    return Json(ApiResponse::ok("Restored".to_string()));
                }
                Err(e) => {
                    std::fs::remove_file(&temp_path).ok();
                    return Json(ApiResponse::err(format!("Restore failed: {}", e)));
                }
            }
        }
    }
    Json(ApiResponse::err("No file uploaded"))
}

pub async fn check_scheduled_backup(state: Arc<AppState>) {
    let enabled = state.db.get_setting("backup_enabled").unwrap_or_default() == "true";
    if !enabled { return; }

    let backup_time = state.db.get_setting("backup_time").unwrap_or_else(|| "02:00".to_string());
    let backup_day = state.db.get_setting("backup_day").unwrap_or_else(|| "daily".to_string());
    let last_run = state.db.get_setting("backup_last_run").unwrap_or_default();

    let now = now_in_configured_tz(&state);
    let today = now.format("%Y-%m-%d").to_string();
    if last_run == today { return; }

    if backup_day != "daily" {
        let weekday = now.format("%u").to_string();
        if weekday != backup_day { return; }
    }

    let current_time = now.format("%H:%M").to_string();
    if current_time < backup_time { return; }

    let dir = get_backup_dir(&state);
    let filename = format!("dockpit_{}.db", now.format("%Y-%m-%d_%H-%M-%S"));
    let full_path = format!("{}/{}", dir, filename);

    match state.db.backup_to(&full_path) {
        Ok(_) => {
            let retention: usize = state.db.get_setting("backup_retention")
                .and_then(|v| v.parse().ok()).unwrap_or(7);
            enforce_retention(&dir, retention);
            state.db.set_setting("backup_last_run", &today).ok();
            notify_and_email(&state, "backup_success", "Scheduled backup created", &filename);
            state.db.log_audit("system", "backup_scheduled", Some(&filename), None);
            tracing::info!("Scheduled backup created: {}", filename);
        }
        Err(e) => {
            notify_and_email(&state, "backup_failed", "Scheduled backup failed", &e);
            tracing::error!("Scheduled backup failed: {}", e);
        }
    }
}
