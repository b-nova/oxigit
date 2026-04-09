use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub display_name: String,
    pub is_admin: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: i64,
    pub username: String,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Repository {
    pub id: i64,
    pub owner_id: i64,
    pub name: String,
    pub description: String,
    pub is_private: bool,
    pub forked_from: Option<i64>,
    pub has_remix: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight repository entry from the global index (control DB).
/// Used for cross-tenant listing/search without loading full tenant DBs.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RepositoryIndexEntry {
    pub id: i64,
    pub org_slug: String,
    pub owner_id: i64,
    pub owner_username: String,
    pub repo_name: String,
    pub description: String,
    pub is_private: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SshKey {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub public_key: String,
    pub fingerprint: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Collaborator {
    pub id: i64,
    pub repo_id: i64,
    pub user_id: i64,
    pub permission: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Issue {
    pub id: i64,
    pub repo_id: i64,
    pub number: i64,
    pub title: String,
    pub description: String,
    pub author_id: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IssueComment {
    pub id: i64,
    pub issue_id: i64,
    pub author_id: i64,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PullRequest {
    pub id: i64,
    pub repo_id: i64,
    pub number: i64,
    pub title: String,
    pub description: String,
    pub author_id: i64,
    pub source_branch: String,
    pub target_branch: String,
    pub status: String,
    pub merged_by: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiCommitMetadata {
    pub id: i64,
    pub repo_id: i64,
    pub commit_sha: String,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub ai_prompt: Option<String>,
    pub ai_session_id: Option<String>,
    pub ai_files_touched: Option<String>,
    pub ai_prompt_index: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AiDiffSummary {
    pub id: i64,
    pub repo_id: i64,
    pub commit_sha: String,
    pub summary: String,
    pub risk_flags: Option<String>,
    pub generated_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserSettings {
    pub id: i64,
    pub user_id: i64,
    pub llm_provider: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
    pub llm_base_url: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RepoWebhook {
    pub id: i64,
    pub repo_id: i64,
    pub url: String,
    pub secret: Option<String>,
    pub events: String,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MergeConflict {
    pub id: i64,
    pub repo_id: i64,
    pub user_id: i64,
    pub operation_type: String,
    pub target_ref: String,
    pub source_ref: String,
    pub merge_base: String,
    pub auto_tree: Option<String>,
    pub context_json: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MergeConflictFile {
    pub id: i64,
    pub merge_conflict_id: i64,
    pub file_path: String,
    pub conflict_type: String,
    pub resolution: Option<String>,
    pub resolved_content: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuardrailRule {
    pub id: i64,
    pub repo_id: i64,
    pub category: String,
    pub action: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuardrailConfig {
    pub id: i64,
    pub repo_id: i64,
    pub min_vibe_score: Option<i64>,
    pub max_files_per_push: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuardrailViolation {
    pub id: i64,
    pub repo_id: i64,
    pub commit_sha: String,
    pub ref_name: Option<String>,
    pub rule_category: String,
    pub action_taken: String,
    pub severity: String,
    pub message: String,
    pub file_path: Option<String>,
    pub pushed_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Recipe {
    pub id: i64,
    pub repo_id: i64,
    pub session_id: String,
    pub author_id: i64,
    pub title: String,
    pub description: String,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub tags: Option<String>,
    pub prompt_count: i64,
    pub file_count: i64,
    pub vibe_score: Option<i64>,
    pub replay_count: i64,
    pub is_public: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecipeStep {
    pub id: i64,
    pub recipe_id: i64,
    pub step_order: i64,
    pub prompt_text: Option<String>,
    pub prompt_index: Option<i64>,
    pub commit_message: String,
    pub files_json: Option<String>,
    pub diff_text: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecipeReplay {
    pub id: i64,
    pub recipe_id: i64,
    pub user_id: i64,
    pub target_repo_id: i64,
    pub target_branch: String,
    pub mode: String,
    pub status: String,
    pub steps_applied: i64,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeployPreview {
    pub id: i64,
    pub repo_id: i64,
    pub commit_sha: String,
    pub branch: String,
    pub preview_url: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Subscription {
    pub id: i64,
    pub user_id: i64,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: Option<String>,
    pub plan: String,
    pub status: String,
    pub current_period_end: Option<String>,
    pub seats: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FoundingMember {
    pub id: i64,
    pub user_id: i64,
    pub slot_number: i64,
    pub claimed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ContactInquiry {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub company: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Organization {
    pub id: i64,
    pub slug: String,
    pub display_name: String,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrgMembership {
    pub id: i64,
    pub org_id: i64,
    pub user_id: i64,
    pub role: String,
    pub created_at: String,
}
