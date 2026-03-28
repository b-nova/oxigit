use std::path::PathBuf;

use leptos::prelude::*;
use leptos_axum::extract;
use sqlx::SqlitePool;

use crate::pages::UserInfo;

/// Application state shared via Leptos context.
#[derive(Clone, Debug)]
pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub leptos_options: LeptosOptions,
}

pub async fn get_pool() -> Result<SqlitePool, ServerFnError> {
    let state = expect_context::<AppState>();
    Ok(state.pool)
}

pub async fn get_data_dir() -> Result<PathBuf, ServerFnError> {
    let state = expect_context::<AppState>();
    Ok(state.data_dir)
}

/// Extract the current user from the session cookie.
pub async fn extract_session_user() -> Option<UserInfo> {
    let headers: axum::http::HeaderMap = extract().await.ok()?;
    let cookie = headers.get("cookie")?.to_str().ok()?;

    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("oxigit_session=") {
            let decoded = value.trim();
            let mut parts = decoded.splitn(2, ':');
            let id: i64 = parts.next()?.parse().ok()?;
            let username = parts.next()?.to_string();
            return Some(UserInfo { id, username });
        }
    }
    None
}

/// Set session cookie on the response.
pub async fn set_session_user(user_id: i64, username: &str) {
    let cookie = format!(
        "oxigit_session={}:{}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800",
        user_id, username
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
