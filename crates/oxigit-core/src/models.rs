use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
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
