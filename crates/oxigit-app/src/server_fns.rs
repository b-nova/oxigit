use std::path::PathBuf;
use std::sync::Arc;

use axum::Extension;
use leptos::prelude::*;
use leptos_axum::extract;
use sqlx::SqlitePool;

use crate::pages::UserInfo;

/// Application state shared via Axum Extension layer.
#[derive(Clone)]
pub struct AppState {
    pub tenant_mgr: Arc<oxigit_core::tenant::TenantPoolManager>,
    pub multi_tenant: bool,
    pub data_dir: PathBuf,
    pub secret_key: Vec<u8>,
    pub leptos_options: LeptosOptions,
    pub llm_provider: String,
    pub llm_api_key: Option<String>,
    pub llm_model: String,
    pub llm_base_url: Option<String>,
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub stripe_price_flat: Option<String>,
    pub stripe_price_team: Option<String>,
    pub stripe_price_founding: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
    pub contact_email: Option<String>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
}

impl AppState {
    /// Backward-compatible pool accessor — returns the control plane pool.
    /// During migration, all existing `get_pool()` callers use this.
    pub fn pool(&self) -> SqlitePool {
        self.tenant_mgr.control_pool().clone()
    }
}

/// Get the control plane pool (users, auth, billing, orgs).
/// This is the backward-compatible default — all existing server functions use this.
pub async fn get_pool() -> Result<SqlitePool, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok(state.pool())
}

/// Get the control plane pool explicitly.
pub async fn get_control_pool() -> Result<SqlitePool, ServerFnError> {
    get_pool().await
}

/// Get the tenant pool for the user's active org.
/// Returns an error if no org is selected in the session.
pub async fn get_tenant_pool() -> Result<SqlitePool, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    let org_slug = extract_active_org()
        .await
        .ok_or_else(|| ServerFnError::new("No active organization"))?;
    state
        .tenant_mgr
        .get_tenant_pool(&org_slug)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Get the tenant pool for a specific repo by looking up its org in the repository index.
/// In legacy (single-DB) mode, returns the control pool for backward compatibility.
pub async fn get_repo_pool(owner: &str, repo: &str) -> Result<SqlitePool, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    if !state.multi_tenant {
        return Ok(state.pool());
    }
    let control = state.pool();
    let org_slug = oxigit_core::db::lookup_repo_org(&control, owner, repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    state
        .tenant_mgr
        .get_tenant_pool(&org_slug)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Get both the control pool and tenant pool for a repo.
/// Callers should use `db::get_repository_cross(&control, &tenant, owner, repo)` to look up
/// a repo when the users table is in the control DB and repositories in the tenant DB.
/// In legacy mode both pools are the same.
pub async fn get_repo_pools(owner: &str, repo: &str) -> Result<(SqlitePool, SqlitePool), ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    let control = state.pool();
    if !state.multi_tenant {
        return Ok((control.clone(), control));
    }
    let org_slug = oxigit_core::db::lookup_repo_org(&control, owner, repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let tenant = state
        .tenant_mgr
        .get_tenant_pool(&org_slug)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok((control, tenant))
}

/// Resolve the filesystem path to a bare repository.
/// In multi-tenant mode, looks up the org via repository_index and returns the tenant-specific path.
/// In legacy mode, returns the standard `{data_dir}/repos/{owner}/{repo}.git` path.
pub async fn get_repo_path(owner: &str, repo: &str) -> Result<std::path::PathBuf, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    if !state.multi_tenant {
        return Ok(oxigit_core::git::repo_path(&state.data_dir, owner, repo));
    }
    let control = state.pool();
    let org_slug = oxigit_core::db::lookup_repo_org(&control, owner, repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(state.tenant_mgr.tenant_repos_dir(&org_slug).join(owner).join(format!("{repo}.git")))
}

/// Get the TenantPoolManager for advanced operations (provisioning, etc.).
pub async fn get_tenant_mgr() -> Result<Arc<oxigit_core::tenant::TenantPoolManager>, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok(state.tenant_mgr.clone())
}

/// Check if multi-tenant mode is active.
pub async fn is_multi_tenant() -> Result<bool, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok(state.multi_tenant)
}

pub async fn get_data_dir() -> Result<PathBuf, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok(state.data_dir)
}

pub async fn get_llm_config() -> Result<(String, Option<String>, String, Option<String>), ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    Ok((state.llm_provider, state.llm_api_key, state.llm_model, state.llm_base_url))
}

/// Resolve effective LLM config: user settings override server defaults.
pub async fn get_effective_llm_config(user_id: Option<i64>) -> Result<(String, Option<String>, String, Option<String>), ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    let pool = state.pool();
    let (mut provider, mut api_key, mut model, mut base_url) =
        (state.llm_provider, state.llm_api_key, state.llm_model, state.llm_base_url);

    if let Some(uid) = user_id {
        if let Ok(Some(settings)) = oxigit_core::db::get_user_settings(&pool, uid).await {
            if let Some(p) = settings.llm_provider { provider = p; }
            if let Some(k) = settings.llm_api_key { api_key = Some(k); }
            if let Some(m) = settings.llm_model { model = m; }
            if let Some(u) = settings.llm_base_url { base_url = Some(u); }
        }
    }

    Ok((provider, api_key, model, base_url))
}

/// Stripe configuration for billing operations.
#[derive(Clone, Debug)]
pub struct StripeConfig {
    pub secret_key: String,
    pub publishable_key: Option<String>,
    pub webhook_secret: String,
    pub price_flat: Option<String>,
    pub price_team: Option<String>,
    pub price_founding: Option<String>,
}

/// Extract Stripe configuration. Returns None if Stripe is not configured.
pub async fn get_stripe_config() -> Result<Option<StripeConfig>, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    match (state.stripe_secret_key, state.stripe_webhook_secret) {
        (Some(secret_key), Some(webhook_secret)) => Ok(Some(StripeConfig {
            secret_key,
            publishable_key: state.stripe_publishable_key,
            webhook_secret,
            price_flat: state.stripe_price_flat,
            price_team: state.stripe_price_team,
            price_founding: state.stripe_price_founding,
        })),
        _ => Ok(None),
    }
}

/// Extract SMTP configuration. Returns None if SMTP is not configured.
pub async fn get_smtp_config() -> Result<Option<oxigit_core::email::SmtpConfig>, ServerFnError> {
    let Extension(state): Extension<AppState> = extract().await?;
    match (&state.smtp_host, &state.smtp_user, &state.smtp_password, &state.smtp_from, &state.contact_email) {
        (Some(host), Some(user), Some(password), Some(from), Some(contact_email)) => {
            Ok(Some(oxigit_core::email::SmtpConfig {
                host: host.clone(),
                port: state.smtp_port,
                user: user.clone(),
                password: password.clone(),
                from: from.clone(),
                contact_email: contact_email.clone(),
            }))
        }
        _ => Ok(None),
    }
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
/// Returns `None` if the cookie is missing/invalid or the user is disabled.
pub async fn extract_session_user() -> Option<UserInfo> {
    let Extension(state): Extension<AppState> = extract().await.ok()?;
    let headers: axum::http::HeaderMap = extract().await.ok()?;
    let cookie = headers.get("cookie")?.to_str().ok()?;

    let mut user_info = None;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("oxigit_session=") {
            user_info = verify_session_cookie(value.trim(), &state.secret_key);
            break;
        }
    }

    let user = user_info?;

    // Block disabled users
    if oxigit_core::db::is_user_disabled(&state.pool(), user.id)
        .await
        .unwrap_or(false)
    {
        return None;
    }

    Some(user)
}

/// Require the current user to be an admin. Returns the user or a ServerFnError.
pub async fn require_admin() -> Result<UserInfo, ServerFnError> {
    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    if !oxigit_core::db::is_user_admin(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        return Err(ServerFnError::new("Admin access required"));
    }
    Ok(user)
}

/// Get AI access level for a user (Limited for free, Full for paid plans).
pub async fn get_ai_access_level(
    user_id: i64,
) -> Result<oxigit_core::entitlements::AiAccessLevel, ServerFnError> {
    let pool = get_pool().await?;
    let plan = oxigit_core::db::get_user_plan(&pool, user_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(oxigit_core::entitlements::ai_access_for_plan(&plan))
}

/// Get plan entitlements for a user based on their active subscription.
pub async fn get_user_entitlements(
    user_id: i64,
) -> Result<oxigit_core::entitlements::PlanEntitlements, ServerFnError> {
    let pool = get_pool().await?;
    let plan = oxigit_core::db::get_user_plan(&pool, user_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(oxigit_core::entitlements::for_plan(&plan))
}

/// Set a signed session cookie on the response.
pub async fn set_session_user(user_id: i64, username: &str, org_slug: Option<&str>) {
    let Extension(state): Extension<AppState> = match extract().await {
        Ok(s) => s,
        Err(_) => return,
    };
    let signed = sign_session_cookie(user_id, username, org_slug, &state.secret_key);
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

/// Update only the active org in the session cookie (for org switching).
pub async fn set_session_org(user: &UserInfo, org_slug: &str) {
    set_session_user(user.id, &user.username, Some(org_slug)).await;
}

/// Extract the active org slug from the session, if set.
pub async fn extract_active_org() -> Option<String> {
    let user = extract_session_user().await?;
    user.active_org_slug
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

/// Sign a session value: "user_id:username:org_slug:hmac_hex"
/// When org_slug is None, format is "user_id:username:hmac_hex" (backward compat).
fn sign_session_cookie(user_id: i64, username: &str, org_slug: Option<&str>, secret: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = match org_slug {
        Some(slug) => format!("{}:{}:{}", user_id, username, slug),
        None => format!("{}:{}", user_id, username),
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC key error");
    mac.update(payload.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    format!("{}:{}", payload, signature)
}

/// Verify a signed session cookie. Returns None if invalid.
/// Supports both old format "user_id:username:hmac" and new "user_id:username:org_slug:hmac".
fn verify_session_cookie(value: &str, secret: &[u8]) -> Option<UserInfo> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    // Signature is always the last colon-separated segment
    let mut parts = value.rsplitn(2, ':');
    let signature_hex = parts.next()?;
    let payload = parts.next()?;

    // Verify HMAC
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    mac.update(payload.as_bytes());
    let expected_sig = hex::decode(signature_hex).ok()?;
    mac.verify_slice(&expected_sig).ok()?;

    // Parse payload: "user_id:username" or "user_id:username:org_slug"
    let payload_parts: Vec<&str> = payload.splitn(3, ':').collect();
    match payload_parts.len() {
        2 => {
            let id: i64 = payload_parts[0].parse().ok()?;
            let username = payload_parts[1].to_string();
            Some(UserInfo { id, username, active_org_slug: None })
        }
        3 => {
            let id: i64 = payload_parts[0].parse().ok()?;
            let username = payload_parts[1].to_string();
            let org_slug = payload_parts[2].to_string();
            Some(UserInfo {
                id,
                username,
                active_org_slug: if org_slug.is_empty() { None } else { Some(org_slug) },
            })
        }
        _ => None,
    }
}
