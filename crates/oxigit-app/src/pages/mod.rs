pub mod home;
pub mod login;
pub mod register;
pub mod repo_list;
pub mod repo_new;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoInfo {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_private: bool,
    pub created_at: String,
}

#[server]
pub async fn get_current_user() -> Result<Option<UserInfo>, ServerFnError> {
    use crate::server_fns::extract_session_user;
    Ok(extract_session_user().await)
}

#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server_fns::clear_session;
    clear_session().await;
    leptos_axum::redirect("/");
    Ok(())
}
