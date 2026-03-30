pub mod ai_timeline;
pub mod commit_view;
pub mod explore;
pub mod commits;
pub mod home;
pub mod issue_list;
pub mod issue_new;
pub mod issue_view;
pub mod pr_list;
pub mod pr_new;
pub mod pr_view;
pub mod login;
pub mod register;
pub mod repo_blob;
pub mod repo_list;
pub mod repo_new;
pub mod repo_settings;
pub mod repo_view;
pub mod settings;
pub mod user_profile;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeEntryInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitSummary {
    pub message: String,
    pub author: String,
    pub time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoTreeResponse {
    pub info: RepoInfo,
    pub entries: Vec<TreeEntryInfo>,
    pub commit: Option<CommitSummary>,
    pub branches: Vec<String>,
    pub current_ref: String,
    pub readme_html: Option<String>,
    pub forked_from: Option<String>,
    pub can_fork: bool,
    pub is_owner: bool,
    pub base_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiMetadataInfo {
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub ai_prompt: Option<String>,
    pub ai_session_id: Option<String>,
    pub ai_files_touched: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiTimelineEntry {
    pub commit_sha: String,
    pub short_sha: String,
    pub commit_message: String,
    pub commit_author: String,
    pub commit_time: String,
    pub metadata: AiMetadataInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiTimelineSession {
    pub session_id: Option<String>,
    pub ai_tool: String,
    pub entries: Vec<AiTimelineEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiTimelineResponse {
    pub owner: String,
    pub repo: String,
    pub sessions: Vec<AiTimelineSession>,
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
