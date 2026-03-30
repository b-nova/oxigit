use std::path::PathBuf;

use axum::Extension;
use leptos::prelude::*;
use leptos_axum::extract;
use sqlx::SqlitePool;

use crate::pages::UserInfo;

/// Application state shared via Axum Extension layer.
#[derive(Clone, Debug)]
pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub secret_key: Vec<u8>,
    pub leptos_options: LeptosOptions,
    pub llm_provider: String,
    pub llm_api_key: Option<String>,
    pub llm_model: String,
    pub llm_base_url: Option<String>,
}

pub async fn get_pool() -> Result<SqlitePool, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok(state.pool)
}

pub async fn get_data_dir() -> Result<PathBuf, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok(state.data_dir)
}

pub async fn get_llm_config() -> Result<(String, Option<String>, String, Option<String>), ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok((state.llm_provider, state.llm_api_key, state.llm_model, state.llm_base_url))
}

/// Extract the base URL from the request Host header.
pub async fn get_base_url() -> String {
    let headers: axum::http::HeaderMap = match extract().await {
        Ok(h) => h,
        Err(_) => return String::new(),
    };
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    // Determine scheme — if behind a proxy, check X-Forwarded-Proto
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");
    format!("{}://{}", scheme, host)
}

/// Extract the current user from the signed session cookie.
pub async fn extract_session_user() -> Option<UserInfo> {
    let Extension(state): Extension<AppState> = extract().await.ok()?;
    let headers: axum::http::HeaderMap = extract().await.ok()?;
    let cookie = headers.get("cookie")?.to_str().ok()?;

    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("oxigit_session=") {
            return verify_session_cookie(value.trim(), &state.secret_key);
        }
    }
    None
}

/// Set a signed session cookie on the response.
pub async fn set_session_user(user_id: i64, username: &str) {
    let Extension(state): Extension<AppState> = match extract().await {
        Ok(s) => s,
        Err(_) => return,
    };
    let signed = sign_session_cookie(user_id, username, &state.secret_key);
    let cookie = format!(
        "oxigit_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800",
        signed
    );
    let opts = expect_context::<leptos_axum::ResponseOptions>();
    opts.insert_header(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&cookie).unwrap(),
    );
}

/// Clear session cookie.
pub async fn clear_session() {
    let cookie = "oxigit_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
    let opts = expect_context::<leptos_axum::ResponseOptions>();
    opts.insert_header(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_str(cookie).unwrap(),
    );
}

/// Sign a session value: "user_id:username:hmac_hex"
fn sign_session_cookie(user_id: i64, username: &str, secret: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!("{}:{}", user_id, username);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC key error");
    mac.update(payload.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    format!("{}:{}", payload, signature)
}

/// Verify a signed session cookie. Returns None if invalid.
fn verify_session_cookie(value: &str, secret: &[u8]) -> Option<UserInfo> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    // Format: "user_id:username:signature_hex"
    let mut parts = value.rsplitn(2, ':');
    let signature_hex = parts.next()?;
    let payload = parts.next()?;

    // Verify HMAC
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    mac.update(payload.as_bytes());
    let expected_sig = hex::decode(signature_hex).ok()?;
    mac.verify_slice(&expected_sig).ok()?;

    // Parse payload
    let mut payload_parts = payload.splitn(2, ':');
    let id: i64 = payload_parts.next()?.parse().ok()?;
    let username = payload_parts.next()?.to_string();

    Some(UserInfo { id, username })
}
