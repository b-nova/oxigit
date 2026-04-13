pub mod admin;
pub mod ai_hub;
pub mod ai_session_detail;
pub mod billing;
pub mod billing_details;
pub mod blame;
pub mod commit_view;
pub mod commits;
pub mod compare;
pub mod conflict_resolve;
pub mod contact;
pub mod explore;
pub mod home;
pub mod issue_list;
pub mod issue_new;
pub mod issue_view;
pub mod login;
pub mod org_new;
pub mod org_settings;
pub mod pr_list;
pub mod pr_new;
pub mod pr_view;
pub mod pricing;
pub mod profile_edit;
pub mod prompt_detail;
pub mod prompt_history;
pub mod recipe_detail;
pub mod recipe_marketplace;
pub mod recipe_share;
pub mod register;
pub mod remix_guide;
pub mod repo_blob;
pub mod repo_list;
pub mod repo_metrics;
pub mod repo_new;
pub mod repo_settings;
pub mod repo_view;
pub mod settings;
pub mod user_profile;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
pub fn render_diff(diff: &str) -> String {
    use std::fmt::Write;
    let mut html = String::new();
    let mut in_file = false;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            if in_file {
                html.push_str("</pre></div>");
            }
            in_file = true;
            let _ = write!(
                html,
                r#"<div class="diff-file"><div class="diff-header">{}</div><pre class="diff-content">"#,
                escape_html(line)
            );
        } else if line.starts_with("+++") || line.starts_with("---") {
            let _ = write!(
                html,
                r#"<span class="diff-meta">{}</span>"#,
                escape_html(line)
            );
            html.push('\n');
        } else if line.starts_with("@@") {
            let _ = write!(
                html,
                r#"<span class="diff-hunk">{}</span>"#,
                escape_html(line)
            );
            html.push('\n');
        } else if line.starts_with('+') {
            let _ = write!(
                html,
                r#"<span class="diff-add">{}</span>"#,
                escape_html(line)
            );
            html.push('\n');
        } else if line.starts_with('-') {
            let _ = write!(
                html,
                r#"<span class="diff-del">{}</span>"#,
                escape_html(line)
            );
            html.push('\n');
        } else {
            let _ = write!(html, "{}", escape_html(line));
            html.push('\n');
        }
    }

    if in_file {
        html.push_str("</pre></div>");
    }
    html
}

#[cfg(feature = "ssr")]
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub active_org_slug: Option<String>,
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
    #[serde(default)]
    pub ai_prompt_index: Option<i64>,
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
    pub vibe_score: Option<VibeScoreInfo>,
    pub recipe_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionListItem {
    pub session_id: String,
    pub ai_tool: String,
    pub commit_count: i64,
    pub first_prompt: Option<String>,
    pub first_time: String,
    pub last_time: String,
    pub vibe_score: Option<u8>,
    pub vibe_grade: Option<String>,
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
    pub user_vibe_score: Option<u8>,
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
    #[serde(default)]
    pub is_limited: bool,
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
    pub summary: Option<DiffSummaryInfo>,
    pub vibe_score: Option<VibeScoreInfo>,
    pub first_time: String,
    pub last_time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VibeScoreInfo {
    pub score: u8,
    pub grade: String,
    pub commits_per_prompt: f64,
    pub risk_density: f64,
    pub churn_ratio: f64,
    pub file_scope: f64,
    pub was_reverted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionScoreItem {
    pub session_id: String,
    pub ai_tool: String,
    pub score: u8,
    pub grade: String,
    pub commit_count: i64,
    pub prompt_count: i64,
    pub first_time: String,
    pub last_time: String,
    pub first_prompt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolScoreInfo {
    pub ai_tool: String,
    pub session_count: i64,
    pub average_score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoMetricsResponse {
    pub owner: String,
    pub repo: String,
    pub average_score: f64,
    pub total_sessions: i64,
    pub total_prompts: i64,
    pub session_scores: Vec<SessionScoreItem>,
    pub tool_comparison: Vec<ToolScoreInfo>,
    pub risk_distribution: Vec<RiskCountInfo>,
    #[serde(default)]
    pub data_range_days: Option<u32>,
    #[serde(default)]
    pub is_limited: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeListItem {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub tags: Vec<String>,
    pub prompt_count: i64,
    pub file_count: i64,
    pub vibe_score: Option<i64>,
    pub replay_count: i64,
    pub author: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeFileInfo {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeStepInfo {
    pub step_order: i64,
    pub prompt_text: Option<String>,
    pub commit_message: String,
    pub files: Vec<RecipeFileInfo>,
    pub diff_html: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayTargetRepo {
    pub owner: String,
    pub name: String,
    pub branches: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeDetailResponse {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub tags: Vec<String>,
    pub vibe_score: Option<i64>,
    pub replay_count: i64,
    pub author: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub steps: Vec<RecipeStepInfo>,
    pub can_replay: bool,
    pub user_repos: Vec<ReplayTargetRepo>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeMarketplaceResponse {
    pub recipes: Vec<RecipeListItem>,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuardrailRuleInfo {
    pub category: String,
    pub action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuardrailSettingsInfo {
    pub rules: Vec<GuardrailRuleInfo>,
    pub min_vibe_score: Option<i64>,
    pub max_files_per_push: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViolationInfo {
    pub id: i64,
    pub commit_sha: String,
    pub short_sha: String,
    pub category: String,
    pub action_taken: String,
    pub severity: String,
    pub message: String,
    pub file_path: Option<String>,
    pub created_at: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrgListItem {
    pub slug: String,
    pub display_name: String,
    pub role: String,
}

#[server]
#[allow(clippy::needless_return)]
pub async fn list_my_orgs() -> Result<Vec<OrgListItem>, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{get_control_pool, require_auth, sfn_err};
        use oxigit_core::db;

        let user = require_auth().await?;
        let pool = get_control_pool().await?;
        let orgs = db::list_user_organizations(&pool, user.id)
            .await
            .map_err(sfn_err)?;
        return Ok(orgs
            .into_iter()
            .map(|(org, role)| OrgListItem {
                slug: org.slug,
                display_name: org.display_name,
                role,
            })
            .collect());
    }
    #[cfg(not(feature = "saas"))]
    Ok(vec![])
}

#[server]
#[allow(clippy::needless_return)]
pub async fn switch_org(slug: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{get_control_pool, require_auth, set_session_org, sfn_err};
        use oxigit_core::db;

        let user = require_auth().await?;
        let pool = get_control_pool().await?;

        let org = db::get_organization_by_slug(&pool, &slug)
            .await
            .map_err(sfn_err)?;
        if !db::is_org_member(&pool, org.id, user.id)
            .await
            .map_err(sfn_err)?
        {
            return Err(ServerFnError::new("Not a member of this organization"));
        }

        set_session_org(&user, &slug).await;
        leptos_axum::redirect("/repos");
        return Ok(());
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = slug;
        Err(ServerFnError::new("Organizations not available"))
    }
}

#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server_fns::clear_session;
    clear_session().await;
    leptos_axum::redirect("/");
    Ok(())
}
