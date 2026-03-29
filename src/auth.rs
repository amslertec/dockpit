use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

use crate::models::Claims;

const JWT_SECRET_ENV: &str = "DOCKPIT_JWT_SECRET";

fn get_secret() -> String {
    match std::env::var(JWT_SECRET_ENV) {
        Ok(secret) if secret.len() >= 16 => secret,
        Ok(_) => {
            tracing::error!("DOCKPIT_JWT_SECRET is too short (min 16 chars). Using insecure fallback!");
            "dockpit-insecure-dev-only-change-me".to_string()
        }
        Err(_) => {
            tracing::error!("DOCKPIT_JWT_SECRET not set! Using insecure fallback. Set this in production!");
            "dockpit-insecure-dev-only-change-me".to_string()
        }
    }
}

pub fn create_token(user_id: &str, username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(2))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_secret().as_bytes()),
    )
}

pub fn check_role(role: &str, required: &[&str]) -> bool {
    required.contains(&role)
}

pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(get_secret().as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/// Base auth - any logged-in user (viewer+)
pub async fn auth_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            match validate_token(token) {
                Ok(_claims) => Ok(next.run(request).await),
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Editor+ auth - editor, admin, super_admin (can start/stop containers)
pub async fn editor_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let claims = extract_claims_from_request(&request)?;
    if check_role(&claims.role, &["super_admin", "admin", "editor"]) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Admin+ auth - admin, super_admin (can create/delete/modify resources)
pub async fn admin_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let claims = extract_claims_from_request(&request)?;
    if check_role(&claims.role, &["super_admin", "admin"]) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Super admin only - user management
pub async fn super_admin_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let claims = extract_claims_from_request(&request)?;
    if check_role(&claims.role, &["super_admin"]) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn extract_claims_from_request(request: &Request) -> Result<Claims, StatusCode> {
    let header = request.headers().get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let token = header.strip_prefix("Bearer ").ok_or(StatusCode::UNAUTHORIZED)?;
    validate_token(token).map_err(|_| StatusCode::UNAUTHORIZED)
}

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, 10)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

pub fn generate_totp_setup(username: &str) -> (String, String, String) {
    use totp_rs::{Algorithm, TOTP, Secret};

    let secret = Secret::generate_secret();
    let secret_base32 = secret.to_encoded().to_string();

    let totp = TOTP::new(
        Algorithm::SHA1, 6, 1, 30,
        secret.to_bytes().unwrap(),
        Some("DockPit".to_string()),
        username.to_string(),
    ).unwrap();

    let otpauth_url = totp.get_url();
    let qr_code = totp.get_qr_base64().unwrap_or_default();

    (secret_base32, otpauth_url, qr_code)
}

pub fn verify_totp(secret_base32: &str, code: &str) -> bool {
    use totp_rs::{Algorithm, TOTP, Secret};

    let secret = match Secret::Encoded(secret_base32.to_string()).to_bytes() {
        Ok(b) => b,
        Err(_) => return false,
    };

    let totp = match TOTP::new(Algorithm::SHA1, 6, 1, 30, secret, Some("DockPit".to_string()), String::new()) {
        Ok(t) => t,
        Err(_) => return false,
    };

    totp.check_current(code).unwrap_or(false)
}

pub fn generate_agent_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}
