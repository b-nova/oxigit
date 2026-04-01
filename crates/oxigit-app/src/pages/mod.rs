pub mod ai_hub;
pub mod ai_session_detail;
pub mod blame;
pub mod commit_view;
pub mod conflict_resolve;
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
pub mod prompt_detail;
pub mod prompt_history;
pub mod register;
pub mod remix_guide;
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
    pub remix_html: Option<String>,
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
    pub diff_html: Option<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskFlagInfo {
    pub category: String,
    pub message: String,
    pub file: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffSummaryInfo {
    pub summary: String,
    pub risk_flags: Vec<RiskFlagInfo>,
    pub generated_by: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffReviewData {
    pub cached_summary: Option<DiffSummaryInfo>,
    pub risk_flags: Vec<RiskFlagInfo>,
    pub llm_available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionDetailResponse {
    pub session_id: String,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub entries: Vec<AiTimelineEntry>,
    pub diff_html: String,
    pub files_changed: Vec<String>,
    pub first_time: String,
    pub last_time: String,
    pub summary: Option<DiffSummaryInfo>,
    pub can_revert: bool,
    pub branches: Vec<String>,
    pub default_branch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionListItem {
    pub session_id: String,
    pub ai_tool: String,
    pub commit_count: i64,
    pub first_prompt: Option<String>,
    pub first_time: String,
    pub last_time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionListItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardData {
    pub total_ai_commits: i64,
    pub total_sessions: i64,
    pub total_repos_with_ai: i64,
    pub tool_usage: Vec<ToolUsageInfo>,
    pub recent_sessions: Vec<RecentSessionInfo>,
    pub risk_counts: Vec<RiskCountInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolUsageInfo {
    pub ai_tool: String,
    pub commit_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentSessionInfo {
    pub session_id: String,
    pub ai_tool: String,
    pub repo_name: String,
    pub commit_count: i64,
    pub last_time: String,
    pub first_prompt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskCountInfo {
    pub category: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptCommitInfo {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: String,
    pub time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptHistoryEntry {
    pub prompt_text: Option<String>,
    pub session_id: Option<String>,
    pub prompt_index: Option<i64>,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub commit_count: i64,
    pub commits: Vec<PromptCommitInfo>,
    pub first_time: String,
    pub last_time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptHistoryResponse {
    pub entries: Vec<PromptHistoryEntry>,
    pub total_prompts: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptDetailResponse {
    pub session_id: String,
    pub prompt_index: i64,
    pub prompt_text: Option<String>,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub commits: Vec<AiTimelineEntry>,
    pub diff_html: String,
    pub files_changed: Vec<String>,
    pub can_operate: bool,
    pub branches: Vec<String>,
    pub default_branch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlameLineInfo {
    pub line_number: usize,
    pub content: String,
    pub commit_sha: String,
    pub short_sha: String,
    pub author: String,
    pub time: String,
    pub is_ai: bool,
    pub ai_tool: Option<String>,
    pub ai_prompt: Option<String>,
    pub ai_session_id: Option<String>,
    pub ai_prompt_index: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlameResponse {
    pub file_path: String,
    pub file_name: String,
    pub lines: Vec<BlameLineInfo>,
    pub ai_line_count: usize,
    pub total_line_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetailResponse {
    pub id: i64,
    pub operation_type: String,
    pub target_ref: String,
    pub source_ref: String,
    pub status: String,
    pub files: Vec<ConflictFileInfo>,
    pub all_resolved: bool,
    pub owner: String,
    pub repo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictFileInfo {
    pub id: i64,
    pub file_path: String,
    pub conflict_type: String,
    pub resolution: Option<String>,
    pub is_resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictFileContentResponse {
    pub file_path: String,
    pub ours_content: Option<String>,
    pub theirs_content: Option<String>,
    pub resolved_content: Option<String>,
    pub ours_label: String,
    pub theirs_label: String,
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
