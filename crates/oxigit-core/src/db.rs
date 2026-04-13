use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

use crate::auth::{hash_password, validate_repo_name, validate_username, verify_password};
use crate::error::{OxigitError, Result};
use crate::models::{
    AiCommitMetadata, AiDiffSummary, Collaborator, ContactInquiry, DeployPreview, GuardrailConfig,
    GuardrailRule, GuardrailViolation, Issue, IssueComment, MergeConflict, MergeConflictFile,
    PullRequest, Recipe, RecipeReplay, RecipeStep, RepoWebhook, Repository, SshKey, User,
    UserSettings,
};
#[cfg(feature = "saas")]
use crate::models::{FoundingMember, OrgMembership, Organization, Subscription};

const MAX_DB_CONNECTIONS: u32 = 5;

/// Map a SQLx UNIQUE constraint violation to an OxigitError::InvalidInput.
fn map_unique_constraint(e: sqlx::Error, message: &str) -> OxigitError {
    match &e {
        sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE") => {
            OxigitError::InvalidInput(message.into())
        }
        _ => e.into(),
    }
}

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_DB_CONNECTIONS)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Ensure the schema exists on a single database (legacy mode).
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(SCHEMA_SQL).execute(pool).await.map_err(|e| {
        OxigitError::Database(sqlx::Error::Protocol(format!(
            "Schema initialization failed: {e}"
        )))
    })?;
    Ok(())
}

/// Ensure the control-plane schema exists (users, auth, billing, orgs).
#[cfg(feature = "saas")]
pub async fn run_control_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(CONTROL_SCHEMA_SQL)
        .execute(pool)
        .await
        .map_err(|e| {
            OxigitError::Database(sqlx::Error::Protocol(format!(
                "Control schema initialization failed: {e}"
            )))
        })?;
    Ok(())
}

/// Ensure the tenant schema exists (repos, issues, PRs, AI data, etc.).
#[cfg(feature = "saas")]
pub async fn run_tenant_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(TENANT_SCHEMA_SQL)
        .execute(pool)
        .await
        .map_err(|e| {
            OxigitError::Database(sqlx::Error::Protocol(format!(
                "Tenant schema initialization failed: {e}"
            )))
        })?;
    Ok(())
}

/// Full schema for the legacy single-database mode.
const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT NOT NULL UNIQUE,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    is_admin        INTEGER NOT NULL DEFAULT 0,
    is_disabled     INTEGER NOT NULL DEFAULT 0,
    display_name    TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

CREATE TABLE IF NOT EXISTS repositories (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id        INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    is_private      INTEGER NOT NULL DEFAULT 0,
    forked_from     INTEGER REFERENCES repositories(id) ON DELETE SET NULL,
    has_remix       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(owner_id, name)
);
CREATE INDEX IF NOT EXISTS idx_repos_owner ON repositories(owner_id);

CREATE TABLE IF NOT EXISTS ssh_keys (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    public_key      TEXT NOT NULL,
    fingerprint     TEXT NOT NULL UNIQUE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_ssh_keys_user ON ssh_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_ssh_keys_fingerprint ON ssh_keys(fingerprint);

CREATE TABLE IF NOT EXISTS collaborators (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission      TEXT NOT NULL DEFAULT 'write',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_collaborators_repo ON collaborators(repo_id);
CREATE INDEX IF NOT EXISTS idx_collaborators_user ON collaborators(user_id);

CREATE TABLE IF NOT EXISTS pull_requests (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    number          INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    author_id       INTEGER NOT NULL REFERENCES users(id),
    source_branch   TEXT NOT NULL,
    target_branch   TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'open',
    merged_by       INTEGER REFERENCES users(id),
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, number)
);
CREATE INDEX IF NOT EXISTS idx_pr_repo ON pull_requests(repo_id);
CREATE INDEX IF NOT EXISTS idx_pr_author ON pull_requests(author_id);
CREATE INDEX IF NOT EXISTS idx_pr_status ON pull_requests(repo_id, status);

CREATE TABLE IF NOT EXISTS issues (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    number          INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    author_id       INTEGER NOT NULL REFERENCES users(id),
    status          TEXT NOT NULL DEFAULT 'open',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, number)
);
CREATE INDEX IF NOT EXISTS idx_issues_repo ON issues(repo_id);
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(repo_id, status);

CREATE TABLE IF NOT EXISTS issue_comments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id        INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    author_id       INTEGER NOT NULL REFERENCES users(id),
    body            TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_issue_comments_issue ON issue_comments(issue_id);

CREATE TABLE IF NOT EXISTS ai_commit_metadata (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    ai_tool         TEXT NOT NULL,
    ai_model        TEXT,
    ai_prompt       TEXT,
    ai_session_id   TEXT,
    ai_files_touched TEXT,
    ai_prompt_index INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, commit_sha)
);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_repo ON ai_commit_metadata(repo_id);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_session ON ai_commit_metadata(repo_id, ai_session_id);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_prompt ON ai_commit_metadata(repo_id, ai_session_id, ai_prompt_index);

CREATE TABLE IF NOT EXISTS ai_diff_summaries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    summary         TEXT NOT NULL,
    risk_flags      TEXT,
    generated_by    TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, commit_sha)
);
CREATE INDEX IF NOT EXISTS idx_ai_diff_summaries_repo ON ai_diff_summaries(repo_id);

CREATE TABLE IF NOT EXISTS user_settings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    llm_provider    TEXT,
    llm_api_key     TEXT,
    llm_model       TEXT,
    llm_base_url    TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS repo_webhooks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    secret          TEXT,
    events          TEXT NOT NULL DEFAULT 'push',
    active          INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS deploy_previews (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    branch          TEXT NOT NULL,
    preview_url     TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_deploy_previews_repo ON deploy_previews(repo_id, commit_sha);

CREATE TABLE IF NOT EXISTS merge_conflicts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    operation_type  TEXT NOT NULL,
    target_ref      TEXT NOT NULL,
    source_ref      TEXT NOT NULL,
    merge_base      TEXT NOT NULL,
    auto_tree       TEXT,
    context_json    TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_merge_conflicts_repo ON merge_conflicts(repo_id, user_id, status);

CREATE TABLE IF NOT EXISTS merge_conflict_files (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    merge_conflict_id   INTEGER NOT NULL REFERENCES merge_conflicts(id) ON DELETE CASCADE,
    file_path           TEXT NOT NULL,
    conflict_type       TEXT NOT NULL,
    resolution          TEXT,
    resolved_content    TEXT,
    resolved_at         TEXT
);

CREATE TABLE IF NOT EXISTS guardrail_rules (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    category        TEXT NOT NULL,
    action          TEXT NOT NULL DEFAULT 'off',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, category)
);
CREATE INDEX IF NOT EXISTS idx_guardrail_rules_repo ON guardrail_rules(repo_id);

CREATE TABLE IF NOT EXISTS guardrail_config (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL UNIQUE REFERENCES repositories(id) ON DELETE CASCADE,
    min_vibe_score  INTEGER,
    max_files_per_push INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS guardrail_violations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    ref_name        TEXT,
    rule_category   TEXT NOT NULL,
    action_taken    TEXT NOT NULL,
    severity        TEXT NOT NULL DEFAULT 'medium',
    message         TEXT NOT NULL,
    file_path       TEXT,
    pushed_by       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_guardrail_violations_repo ON guardrail_violations(repo_id);
CREATE INDEX IF NOT EXISTS idx_guardrail_violations_sha ON guardrail_violations(repo_id, commit_sha);

CREATE TABLE IF NOT EXISTS recipes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    session_id      TEXT NOT NULL,
    author_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    ai_tool         TEXT NOT NULL,
    ai_model        TEXT,
    tags            TEXT,
    prompt_count    INTEGER NOT NULL DEFAULT 0,
    file_count      INTEGER NOT NULL DEFAULT 0,
    vibe_score      INTEGER,
    replay_count    INTEGER NOT NULL DEFAULT 0,
    is_public       BOOLEAN NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, session_id)
);
CREATE INDEX IF NOT EXISTS idx_recipes_author ON recipes(author_id);
CREATE INDEX IF NOT EXISTS idx_recipes_public ON recipes(is_public, created_at DESC);

CREATE TABLE IF NOT EXISTS recipe_steps (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    recipe_id       INTEGER NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    step_order      INTEGER NOT NULL,
    prompt_text     TEXT,
    prompt_index    INTEGER,
    commit_message  TEXT NOT NULL,
    files_json      TEXT,
    diff_text       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(recipe_id, step_order)
);
CREATE INDEX IF NOT EXISTS idx_recipe_steps_recipe ON recipe_steps(recipe_id);

CREATE TABLE IF NOT EXISTS recipe_replays (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    recipe_id       INTEGER NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_repo_id  INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    target_branch   TEXT NOT NULL,
    mode            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    steps_applied   INTEGER NOT NULL DEFAULT 0,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_replays_recipe ON recipe_replays(recipe_id);
CREATE INDEX IF NOT EXISTS idx_replays_user ON recipe_replays(user_id);

CREATE TABLE IF NOT EXISTS subscriptions (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id                 INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stripe_customer_id      TEXT NOT NULL,
    stripe_subscription_id  TEXT,
    plan                    TEXT NOT NULL DEFAULT 'flat',
    status                  TEXT NOT NULL DEFAULT 'active',
    current_period_end      TEXT,
    seats                   INTEGER NOT NULL DEFAULT 1,
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_user_id ON subscriptions(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_stripe_customer_id ON subscriptions(stripe_customer_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe_subscription_id ON subscriptions(stripe_subscription_id);

CREATE TABLE IF NOT EXISTS founding_members (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    slot_number     INTEGER NOT NULL,
    claimed_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS organizations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    slug            TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    created_by      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_organizations_slug ON organizations(slug);

CREATE TABLE IF NOT EXISTS org_memberships (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    org_id          INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'member',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(org_id, user_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_org_memberships_org_user ON org_memberships(org_id, user_id);
CREATE INDEX IF NOT EXISTS idx_org_memberships_user_id ON org_memberships(user_id);

CREATE TABLE IF NOT EXISTS repository_index (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    org_slug        TEXT NOT NULL,
    owner_id        INTEGER NOT NULL,
    owner_username  TEXT NOT NULL,
    repo_name       TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    is_private      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(owner_username, repo_name)
);
CREATE INDEX IF NOT EXISTS idx_repo_index_org ON repository_index(org_slug);
CREATE INDEX IF NOT EXISTS idx_repo_index_owner ON repository_index(owner_username);
CREATE INDEX IF NOT EXISTS idx_repo_index_public ON repository_index(is_private, updated_at DESC);

CREATE TABLE IF NOT EXISTS contact_inquiries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    email           TEXT NOT NULL,
    company         TEXT NOT NULL DEFAULT '',
    message         TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// Schema for the control-plane database (multi-tenant mode).
#[cfg(feature = "saas")]
const CONTROL_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT NOT NULL UNIQUE,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    is_admin        INTEGER NOT NULL DEFAULT 0,
    is_disabled     INTEGER NOT NULL DEFAULT 0,
    display_name    TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

CREATE TABLE IF NOT EXISTS ssh_keys (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    public_key      TEXT NOT NULL,
    fingerprint     TEXT NOT NULL UNIQUE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_ssh_keys_user ON ssh_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_ssh_keys_fingerprint ON ssh_keys(fingerprint);

CREATE TABLE IF NOT EXISTS user_settings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    llm_provider    TEXT,
    llm_api_key     TEXT,
    llm_model       TEXT,
    llm_base_url    TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS subscriptions (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id                 INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stripe_customer_id      TEXT NOT NULL,
    stripe_subscription_id  TEXT,
    plan                    TEXT NOT NULL DEFAULT 'free',
    status                  TEXT NOT NULL DEFAULT 'active',
    current_period_end      TEXT,
    seats                   INTEGER NOT NULL DEFAULT 1,
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_user_id ON subscriptions(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_stripe_customer_id ON subscriptions(stripe_customer_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe_subscription_id ON subscriptions(stripe_subscription_id);

CREATE TABLE IF NOT EXISTS founding_members (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    slot_number     INTEGER NOT NULL,
    claimed_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS organizations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    slug            TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    created_by      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_organizations_slug ON organizations(slug);

CREATE TABLE IF NOT EXISTS org_memberships (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    org_id          INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'member',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(org_id, user_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_org_memberships_org_user ON org_memberships(org_id, user_id);
CREATE INDEX IF NOT EXISTS idx_org_memberships_user_id ON org_memberships(user_id);

CREATE TABLE IF NOT EXISTS repository_index (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    org_slug        TEXT NOT NULL,
    owner_id        INTEGER NOT NULL,
    owner_username  TEXT NOT NULL,
    repo_name       TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    is_private      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(owner_username, repo_name)
);
CREATE INDEX IF NOT EXISTS idx_repo_index_org ON repository_index(org_slug);
CREATE INDEX IF NOT EXISTS idx_repo_index_owner ON repository_index(owner_username);
CREATE INDEX IF NOT EXISTS idx_repo_index_public ON repository_index(is_private, updated_at DESC);
"#;

/// Schema for per-tenant databases (multi-tenant mode).
#[cfg(feature = "saas")]
const TENANT_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS repositories (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id        INTEGER NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    is_private      INTEGER NOT NULL DEFAULT 0,
    forked_from     INTEGER,
    has_remix       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(owner_id, name)
);
CREATE INDEX IF NOT EXISTS idx_repos_owner ON repositories(owner_id);

CREATE TABLE IF NOT EXISTS collaborators (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL,
    permission      TEXT NOT NULL DEFAULT 'write',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_collaborators_repo ON collaborators(repo_id);
CREATE INDEX IF NOT EXISTS idx_collaborators_user ON collaborators(user_id);

CREATE TABLE IF NOT EXISTS pull_requests (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    number          INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    author_id       INTEGER NOT NULL,
    source_branch   TEXT NOT NULL,
    target_branch   TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'open',
    merged_by       INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, number)
);
CREATE INDEX IF NOT EXISTS idx_pr_repo ON pull_requests(repo_id);
CREATE INDEX IF NOT EXISTS idx_pr_author ON pull_requests(author_id);
CREATE INDEX IF NOT EXISTS idx_pr_status ON pull_requests(repo_id, status);

CREATE TABLE IF NOT EXISTS issues (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    number          INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    author_id       INTEGER NOT NULL,
    status          TEXT NOT NULL DEFAULT 'open',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, number)
);
CREATE INDEX IF NOT EXISTS idx_issues_repo ON issues(repo_id);
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(repo_id, status);

CREATE TABLE IF NOT EXISTS issue_comments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id        INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    author_id       INTEGER NOT NULL,
    body            TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_issue_comments_issue ON issue_comments(issue_id);

CREATE TABLE IF NOT EXISTS ai_commit_metadata (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    ai_tool         TEXT NOT NULL,
    ai_model        TEXT,
    ai_prompt       TEXT,
    ai_session_id   TEXT,
    ai_files_touched TEXT,
    ai_prompt_index INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, commit_sha)
);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_repo ON ai_commit_metadata(repo_id);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_session ON ai_commit_metadata(repo_id, ai_session_id);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_prompt ON ai_commit_metadata(repo_id, ai_session_id, ai_prompt_index);

CREATE TABLE IF NOT EXISTS ai_diff_summaries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    summary         TEXT NOT NULL,
    risk_flags      TEXT,
    generated_by    TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, commit_sha)
);
CREATE INDEX IF NOT EXISTS idx_ai_diff_summaries_repo ON ai_diff_summaries(repo_id);

CREATE TABLE IF NOT EXISTS repo_webhooks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    secret          TEXT,
    events          TEXT NOT NULL DEFAULT 'push',
    active          INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS deploy_previews (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    branch          TEXT NOT NULL,
    preview_url     TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_deploy_previews_repo ON deploy_previews(repo_id, commit_sha);

CREATE TABLE IF NOT EXISTS merge_conflicts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL,
    operation_type  TEXT NOT NULL,
    target_ref      TEXT NOT NULL,
    source_ref      TEXT NOT NULL,
    merge_base      TEXT NOT NULL,
    auto_tree       TEXT,
    context_json    TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_merge_conflicts_repo ON merge_conflicts(repo_id, user_id, status);

CREATE TABLE IF NOT EXISTS merge_conflict_files (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    merge_conflict_id   INTEGER NOT NULL REFERENCES merge_conflicts(id) ON DELETE CASCADE,
    file_path           TEXT NOT NULL,
    conflict_type       TEXT NOT NULL,
    resolution          TEXT,
    resolved_content    TEXT,
    resolved_at         TEXT
);

CREATE TABLE IF NOT EXISTS guardrail_rules (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    category        TEXT NOT NULL,
    action          TEXT NOT NULL DEFAULT 'off',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, category)
);
CREATE INDEX IF NOT EXISTS idx_guardrail_rules_repo ON guardrail_rules(repo_id);

CREATE TABLE IF NOT EXISTS guardrail_config (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL UNIQUE REFERENCES repositories(id) ON DELETE CASCADE,
    min_vibe_score  INTEGER,
    max_files_per_push INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS guardrail_violations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT NOT NULL,
    ref_name        TEXT,
    rule_category   TEXT NOT NULL,
    action_taken    TEXT NOT NULL,
    severity        TEXT NOT NULL DEFAULT 'medium',
    message         TEXT NOT NULL,
    file_path       TEXT,
    pushed_by       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_guardrail_violations_repo ON guardrail_violations(repo_id);
CREATE INDEX IF NOT EXISTS idx_guardrail_violations_sha ON guardrail_violations(repo_id, commit_sha);

CREATE TABLE IF NOT EXISTS recipes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    session_id      TEXT NOT NULL,
    author_id       INTEGER NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    ai_tool         TEXT NOT NULL,
    ai_model        TEXT,
    tags            TEXT,
    prompt_count    INTEGER NOT NULL DEFAULT 0,
    file_count      INTEGER NOT NULL DEFAULT 0,
    vibe_score      INTEGER,
    replay_count    INTEGER NOT NULL DEFAULT 0,
    is_public       BOOLEAN NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, session_id)
);
CREATE INDEX IF NOT EXISTS idx_recipes_author ON recipes(author_id);
CREATE INDEX IF NOT EXISTS idx_recipes_public ON recipes(is_public, created_at DESC);

CREATE TABLE IF NOT EXISTS recipe_steps (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    recipe_id       INTEGER NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    step_order      INTEGER NOT NULL,
    prompt_text     TEXT,
    prompt_index    INTEGER,
    commit_message  TEXT NOT NULL,
    files_json      TEXT,
    diff_text       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(recipe_id, step_order)
);
CREATE INDEX IF NOT EXISTS idx_recipe_steps_recipe ON recipe_steps(recipe_id);

CREATE TABLE IF NOT EXISTS recipe_replays (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    recipe_id       INTEGER NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    user_id         INTEGER NOT NULL,
    target_repo_id  INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    target_branch   TEXT NOT NULL,
    mode            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    steps_applied   INTEGER NOT NULL DEFAULT 0,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_replays_recipe ON recipe_replays(recipe_id);
CREATE INDEX IF NOT EXISTS idx_replays_user ON recipe_replays(user_id);
"#;

// --- User queries ---

pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    email: &str,
    password: &str,
) -> Result<User> {
    validate_username(username)?;

    if password.len() < 8 {
        return Err(OxigitError::InvalidInput(
            "Password must be at least 8 characters".into(),
        ));
    }

    let password_hash = hash_password(password)?;

    let result = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(username)
    .bind(email)
    .bind(&password_hash)
    .fetch_one(pool)
    .await;

    match result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::Database(ref e)) if e.message().contains("UNIQUE") => {
            let msg = e.message();
            if msg.contains("username") {
                Err(OxigitError::UsernameTaken)
            } else if msg.contains("email") {
                Err(OxigitError::EmailTaken)
            } else {
                Err(OxigitError::UsernameTaken)
            }
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn authenticate_user(pool: &SqlitePool, username: &str, password: &str) -> Result<User> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?
        .ok_or(OxigitError::AuthFailed)?;

    if !verify_password(password, &user.password_hash)? {
        return Err(OxigitError::AuthFailed);
    }

    Ok(user)
}

pub async fn get_user_by_id(pool: &SqlitePool, id: i64) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(OxigitError::NotFound("User not found".into()))
}

pub async fn get_user_by_username(pool: &SqlitePool, username: &str) -> Result<User> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await?
        .ok_or(OxigitError::NotFound("User not found".into()))
}

pub async fn update_user_profile(
    pool: &SqlitePool,
    user_id: i64,
    display_name: &str,
    email: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE users SET display_name = ?, email = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(display_name)
    .bind(email)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

// --- Repository queries ---

pub async fn create_repository(
    pool: &SqlitePool,
    owner_id: i64,
    name: &str,
    description: &str,
    is_private: bool,
    data_dir: &Path,
) -> Result<Repository> {
    validate_repo_name(name)?;

    // Get owner username for the filesystem path
    let owner = get_user_by_id(pool, owner_id).await?;
    let repo_path = data_dir
        .join("repos")
        .join(&owner.username)
        .join(format!("{name}.git"));

    // Create bare repo on disk
    std::fs::create_dir_all(&repo_path)?;
    crate::git::init_bare_repo(&repo_path)?;

    let repo = sqlx::query_as::<_, Repository>(
        "INSERT INTO repositories (owner_id, name, description, is_private) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(owner_id)
    .bind(name)
    .bind(description)
    .bind(is_private)
    .fetch_one(pool)
    .await?;

    Ok(repo)
}

/// Create a repository in a tenant DB where the `users` table is not available.
/// The caller provides the owner username and the repos directory directly.
#[cfg(feature = "saas")]
pub async fn create_repository_in_tenant(
    pool: &SqlitePool,
    owner_id: i64,
    owner_username: &str,
    name: &str,
    description: &str,
    is_private: bool,
    repos_dir: &Path,
) -> Result<Repository> {
    validate_repo_name(name)?;

    let repo_path = repos_dir.join(owner_username).join(format!("{name}.git"));

    std::fs::create_dir_all(&repo_path)?;
    crate::git::init_bare_repo(&repo_path)?;

    let repo = sqlx::query_as::<_, Repository>(
        "INSERT INTO repositories (owner_id, name, description, is_private) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(owner_id)
    .bind(name)
    .bind(description)
    .bind(is_private)
    .fetch_one(pool)
    .await?;

    Ok(repo)
}

pub async fn list_user_repositories(pool: &SqlitePool, owner_id: i64) -> Result<Vec<Repository>> {
    let repos = sqlx::query_as::<_, Repository>(
        "SELECT * FROM repositories WHERE owner_id = ? ORDER BY updated_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(repos)
}

pub async fn get_repository(
    pool: &SqlitePool,
    owner_name: &str,
    repo_name: &str,
) -> Result<(User, Repository)> {
    let owner = get_user_by_username(pool, owner_name).await?;
    let repo = sqlx::query_as::<_, Repository>(
        "SELECT * FROM repositories WHERE owner_id = ? AND name = ?",
    )
    .bind(owner.id)
    .bind(repo_name)
    .fetch_optional(pool)
    .await?
    .ok_or(OxigitError::NotFound("Repository not found".into()))?;
    Ok((owner, repo))
}

/// Get a repository by owner_id directly (tenant-only, no user lookup).
pub async fn get_repository_by_owner_id(
    pool: &SqlitePool,
    owner_id: i64,
    repo_name: &str,
) -> Result<Repository> {
    let repo = sqlx::query_as::<_, Repository>(
        "SELECT * FROM repositories WHERE owner_id = ? AND name = ?",
    )
    .bind(owner_id)
    .bind(repo_name)
    .fetch_optional(pool)
    .await?
    .ok_or(OxigitError::NotFound("Repository not found".into()))?;
    Ok(repo)
}

/// Cross-DB repository lookup: resolves user from control pool, repo from tenant pool.
/// Works against a single DB too (pass the same pool for both).
pub async fn get_repository_cross(
    control_pool: &SqlitePool,
    tenant_pool: &SqlitePool,
    owner_name: &str,
    repo_name: &str,
) -> Result<(User, Repository)> {
    let owner = get_user_by_username(control_pool, owner_name).await?;
    let repo = get_repository_by_owner_id(tenant_pool, owner.id, repo_name).await?;
    Ok((owner, repo))
}

/// Check if a user can access a repository. Public repos are accessible to all.
/// Private repos require the viewer to be the owner.
pub fn can_access_repo(repo: &Repository, viewer_id: Option<i64>) -> bool {
    if !repo.is_private {
        return true;
    }
    viewer_id == Some(repo.owner_id)
}

/// Search public repositories. If query is empty, returns all public repos.
pub async fn search_public_repositories(
    pool: &SqlitePool,
    query: &str,
) -> Result<Vec<(User, Repository)>> {
    let rows = if query.is_empty() {
        sqlx::query_as::<_, (i64, String, String, String, String, i64, String, String, bool, Option<i64>, bool, String, String)>(
            "SELECT u.id, u.username, u.email, u.password_hash, u.display_name, r.id, r.name, r.description, r.is_private, r.forked_from, r.has_remix, r.created_at, r.updated_at \
             FROM repositories r JOIN users u ON r.owner_id = u.id \
             WHERE r.is_private = 0 \
             ORDER BY r.updated_at DESC LIMIT 50",
        )
        .fetch_all(pool)
        .await?
    } else {
        let pattern = format!("%{}%", query);
        sqlx::query_as::<_, (i64, String, String, String, String, i64, String, String, bool, Option<i64>, bool, String, String)>(
            "SELECT u.id, u.username, u.email, u.password_hash, u.display_name, r.id, r.name, r.description, r.is_private, r.forked_from, r.has_remix, r.created_at, r.updated_at \
             FROM repositories r JOIN users u ON r.owner_id = u.id \
             WHERE r.is_private = 0 AND (r.name LIKE ? OR u.username LIKE ? OR r.description LIKE ?) \
             ORDER BY r.updated_at DESC LIMIT 50",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                uid,
                username,
                email,
                pw,
                display_name,
                rid,
                name,
                desc,
                private,
                forked_from,
                has_remix,
                created,
                updated,
            )| {
                (
                    User {
                        id: uid,
                        username,
                        email,
                        password_hash: pw,
                        display_name,
                        is_admin: false,
                        is_disabled: false,
                        created_at: created.clone(),
                        updated_at: updated.clone(),
                    },
                    Repository {
                        id: rid,
                        owner_id: uid,
                        name,
                        description: desc,
                        is_private: private,
                        forked_from,
                        has_remix,
                        created_at: created,
                        updated_at: updated,
                    },
                )
            },
        )
        .collect())
}

// --- SSH Key queries ---

pub async fn add_ssh_key(
    pool: &SqlitePool,
    user_id: i64,
    name: &str,
    public_key: &str,
    fingerprint: &str,
) -> Result<SshKey> {
    let key = sqlx::query_as::<_, SshKey>(
        "INSERT INTO ssh_keys (user_id, name, public_key, fingerprint) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(user_id)
    .bind(name)
    .bind(public_key)
    .bind(fingerprint)
    .fetch_one(pool)
    .await
    .map_err(|e| map_unique_constraint(e, "This SSH key is already registered"))?;
    Ok(key)
}

pub async fn list_ssh_keys(pool: &SqlitePool, user_id: i64) -> Result<Vec<SshKey>> {
    let keys = sqlx::query_as::<_, SshKey>(
        "SELECT * FROM ssh_keys WHERE user_id = ? ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(keys)
}

pub async fn delete_ssh_key(pool: &SqlitePool, key_id: i64, user_id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM ssh_keys WHERE id = ? AND user_id = ?")
        .bind(key_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(OxigitError::NotFound("SSH key not found".into()));
    }
    Ok(())
}

pub async fn find_user_by_ssh_fingerprint(pool: &SqlitePool, fingerprint: &str) -> Result<User> {
    let key = sqlx::query_as::<_, SshKey>("SELECT * FROM ssh_keys WHERE fingerprint = ?")
        .bind(fingerprint)
        .fetch_optional(pool)
        .await?
        .ok_or(OxigitError::AuthFailed)?;

    get_user_by_id(pool, key.user_id).await
}

// --- Fork queries ---

pub async fn fork_repository(
    pool: &SqlitePool,
    source_owner: &str,
    source_name: &str,
    fork_user_id: i64,
    data_dir: &Path,
) -> Result<Repository> {
    let (source_owner_user, source_repo) = get_repository(pool, source_owner, source_name).await?;

    // Can't fork your own repo
    if source_repo.owner_id == fork_user_id {
        return Err(OxigitError::InvalidInput(
            "Cannot fork your own repository".into(),
        ));
    }

    // Can't fork private repos you don't have access to
    if source_repo.is_private && !can_access_repo(&source_repo, Some(fork_user_id)) {
        return Err(OxigitError::NotFound("Repository not found".into()));
    }

    let fork_user = get_user_by_id(pool, fork_user_id).await?;

    // Check if fork already exists
    if get_repository(pool, &fork_user.username, source_name)
        .await
        .is_ok()
    {
        return Err(OxigitError::InvalidInput(
            "You already have a repository with this name".into(),
        ));
    }

    let source_path = crate::git::repo_path(data_dir, &source_owner_user.username, source_name);
    let fork_path = crate::git::repo_path(data_dir, &fork_user.username, source_name);

    // Clone bare repo on disk
    std::fs::create_dir_all(fork_path.parent().unwrap())?;
    let output = std::process::Command::new("git")
        .args(["clone", "--bare"])
        .arg(&source_path)
        .arg(&fork_path)
        .output()?;

    if !output.status.success() {
        return Err(OxigitError::Git(format!(
            "Failed to fork: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let repo = sqlx::query_as::<_, Repository>(
        "INSERT INTO repositories (owner_id, name, description, is_private, forked_from) VALUES (?, ?, ?, 0, ?) RETURNING *",
    )
    .bind(fork_user_id)
    .bind(source_name)
    .bind(&source_repo.description)
    .bind(source_repo.id)
    .fetch_one(pool)
    .await?;

    Ok(repo)
}

/// Cross-pool variant: repos from tenant pool, users from control pool.
pub async fn get_fork_source_cross(
    control_pool: &SqlitePool,
    tenant_pool: &SqlitePool,
    repo: &Repository,
) -> Result<Option<(User, Repository)>> {
    match repo.forked_from {
        Some(source_id) => {
            let source = sqlx::query_as::<_, Repository>("SELECT * FROM repositories WHERE id = ?")
                .bind(source_id)
                .fetch_optional(tenant_pool)
                .await?;
            match source {
                Some(source_repo) => {
                    let owner = get_user_by_id(control_pool, source_repo.owner_id).await?;
                    Ok(Some((owner, source_repo)))
                }
                None => Ok(None),
            }
        }
        None => Ok(None),
    }
}

// --- Collaborator queries ---

pub async fn add_collaborator(
    pool: &SqlitePool,
    repo_id: i64,
    user_id: i64,
    permission: &str,
) -> Result<Collaborator> {
    let collab = sqlx::query_as::<_, Collaborator>(
        "INSERT INTO collaborators (repo_id, user_id, permission) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(user_id)
    .bind(permission)
    .fetch_one(pool)
    .await
    .map_err(|e| map_unique_constraint(e, "User is already a collaborator"))?;
    Ok(collab)
}

/// Cross-pool variant: collaborators from tenant pool, users from control pool.
pub async fn list_collaborators_cross(
    control_pool: &SqlitePool,
    tenant_pool: &SqlitePool,
    repo_id: i64,
) -> Result<Vec<(User, Collaborator)>> {
    let collabs = sqlx::query_as::<_, Collaborator>(
        "SELECT * FROM collaborators WHERE repo_id = ? ORDER BY created_at",
    )
    .bind(repo_id)
    .fetch_all(tenant_pool)
    .await?;

    let mut result = Vec::new();
    for c in collabs {
        let user = get_user_by_id(control_pool, c.user_id).await?;
        result.push((user, c));
    }
    Ok(result)
}

pub async fn remove_collaborator(pool: &SqlitePool, repo_id: i64, user_id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM collaborators WHERE repo_id = ? AND user_id = ?")
        .bind(repo_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(OxigitError::NotFound("Collaborator not found".into()));
    }
    Ok(())
}

/// Check if a user can push to a repository (owner or collaborator with write access).
pub async fn can_push_repo(pool: &SqlitePool, repo: &Repository, user_id: i64) -> Result<bool> {
    if repo.owner_id == user_id {
        return Ok(true);
    }
    let collab = sqlx::query_as::<_, Collaborator>(
        "SELECT * FROM collaborators WHERE repo_id = ? AND user_id = ? AND permission = 'write'",
    )
    .bind(repo.id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(collab.is_some())
}

// --- Pull Request queries ---

pub async fn create_pull_request(
    pool: &SqlitePool,
    repo_id: i64,
    author_id: i64,
    title: &str,
    description: &str,
    source_branch: &str,
    target_branch: &str,
) -> Result<PullRequest> {
    if title.trim().is_empty() {
        return Err(OxigitError::InvalidInput("Title is required".into()));
    }
    if source_branch == target_branch {
        return Err(OxigitError::InvalidInput(
            "Source and target branches must differ".into(),
        ));
    }

    // Get next PR number for this repo
    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(MAX(number), 0) + 1 FROM pull_requests WHERE repo_id = ?")
            .bind(repo_id)
            .fetch_one(pool)
            .await?;
    let number = row.0;

    let pr = sqlx::query_as::<_, PullRequest>(
        "INSERT INTO pull_requests (repo_id, number, title, description, author_id, source_branch, target_branch) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(number)
    .bind(title.trim())
    .bind(description)
    .bind(author_id)
    .bind(source_branch)
    .bind(target_branch)
    .fetch_one(pool)
    .await?;

    Ok(pr)
}

/// Cross-pool variant: PRs from tenant pool, authors from control pool.
pub async fn list_pull_requests_cross(
    control_pool: &SqlitePool,
    tenant_pool: &SqlitePool,
    repo_id: i64,
    status: Option<&str>,
) -> Result<Vec<(PullRequest, User)>> {
    let prs = if let Some(status) = status {
        sqlx::query_as::<_, PullRequest>(
            "SELECT * FROM pull_requests WHERE repo_id = ? AND status = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .bind(status)
        .fetch_all(tenant_pool)
        .await?
    } else {
        sqlx::query_as::<_, PullRequest>(
            "SELECT * FROM pull_requests WHERE repo_id = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .fetch_all(tenant_pool)
        .await?
    };

    let mut result = Vec::new();
    for pr in prs {
        let author = get_user_by_id(control_pool, pr.author_id).await?;
        result.push((pr, author));
    }
    Ok(result)
}

pub async fn get_pull_request(pool: &SqlitePool, repo_id: i64, number: i64) -> Result<PullRequest> {
    sqlx::query_as::<_, PullRequest>("SELECT * FROM pull_requests WHERE repo_id = ? AND number = ?")
        .bind(repo_id)
        .bind(number)
        .fetch_optional(pool)
        .await?
        .ok_or(OxigitError::NotFound("Pull request not found".into()))
}

pub async fn merge_pull_request(
    pool: &SqlitePool,
    repo_id: i64,
    number: i64,
    merged_by: i64,
) -> Result<PullRequest> {
    let pr = get_pull_request(pool, repo_id, number).await?;
    if pr.status != "open" {
        return Err(OxigitError::InvalidInput(format!(
            "Cannot merge: PR is {}",
            pr.status
        )));
    }

    sqlx::query(
        "UPDATE pull_requests SET status = 'merged', merged_by = ?, updated_at = datetime('now') \
         WHERE repo_id = ? AND number = ?",
    )
    .bind(merged_by)
    .bind(repo_id)
    .bind(number)
    .execute(pool)
    .await?;

    get_pull_request(pool, repo_id, number).await
}

pub async fn close_pull_request(
    pool: &SqlitePool,
    repo_id: i64,
    number: i64,
) -> Result<PullRequest> {
    let pr = get_pull_request(pool, repo_id, number).await?;
    if pr.status != "open" {
        return Err(OxigitError::InvalidInput(format!(
            "Cannot close: PR is {}",
            pr.status
        )));
    }

    sqlx::query(
        "UPDATE pull_requests SET status = 'closed', updated_at = datetime('now') \
         WHERE repo_id = ? AND number = ?",
    )
    .bind(repo_id)
    .bind(number)
    .execute(pool)
    .await?;

    get_pull_request(pool, repo_id, number).await
}

// --- Issue queries ---

pub async fn create_issue(
    pool: &SqlitePool,
    repo_id: i64,
    author_id: i64,
    title: &str,
    description: &str,
) -> Result<Issue> {
    if title.trim().is_empty() {
        return Err(OxigitError::InvalidInput("Title is required".into()));
    }

    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE repo_id = ?")
            .bind(repo_id)
            .fetch_one(pool)
            .await?;

    let issue = sqlx::query_as::<_, Issue>(
        "INSERT INTO issues (repo_id, number, title, description, author_id) VALUES (?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(row.0)
    .bind(title.trim())
    .bind(description)
    .bind(author_id)
    .fetch_one(pool)
    .await?;

    Ok(issue)
}

/// Cross-pool variant: issues from tenant pool, authors from control pool.
pub async fn list_issues_cross(
    control_pool: &SqlitePool,
    tenant_pool: &SqlitePool,
    repo_id: i64,
    status: Option<&str>,
) -> Result<Vec<(Issue, User)>> {
    let issues = if let Some(status) = status {
        sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE repo_id = ? AND status = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .bind(status)
        .fetch_all(tenant_pool)
        .await?
    } else {
        sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE repo_id = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .fetch_all(tenant_pool)
        .await?
    };

    let mut result = Vec::new();
    for issue in issues {
        let author = get_user_by_id(control_pool, issue.author_id).await?;
        result.push((issue, author));
    }
    Ok(result)
}

pub async fn get_issue(pool: &SqlitePool, repo_id: i64, number: i64) -> Result<Issue> {
    sqlx::query_as::<_, Issue>("SELECT * FROM issues WHERE repo_id = ? AND number = ?")
        .bind(repo_id)
        .bind(number)
        .fetch_optional(pool)
        .await?
        .ok_or(OxigitError::NotFound("Issue not found".into()))
}

pub async fn close_issue(pool: &SqlitePool, repo_id: i64, number: i64) -> Result<Issue> {
    let issue = get_issue(pool, repo_id, number).await?;
    if issue.status != "open" {
        return Err(OxigitError::InvalidInput("Issue is not open".into()));
    }
    sqlx::query("UPDATE issues SET status = 'closed', updated_at = datetime('now') WHERE repo_id = ? AND number = ?")
        .bind(repo_id).bind(number).execute(pool).await?;
    get_issue(pool, repo_id, number).await
}

pub async fn reopen_issue(pool: &SqlitePool, repo_id: i64, number: i64) -> Result<Issue> {
    let issue = get_issue(pool, repo_id, number).await?;
    if issue.status != "closed" {
        return Err(OxigitError::InvalidInput("Issue is not closed".into()));
    }
    sqlx::query("UPDATE issues SET status = 'open', updated_at = datetime('now') WHERE repo_id = ? AND number = ?")
        .bind(repo_id).bind(number).execute(pool).await?;
    get_issue(pool, repo_id, number).await
}

pub async fn add_issue_comment(
    pool: &SqlitePool,
    issue_id: i64,
    author_id: i64,
    body: &str,
) -> Result<IssueComment> {
    if body.trim().is_empty() {
        return Err(OxigitError::InvalidInput("Comment cannot be empty".into()));
    }
    let comment = sqlx::query_as::<_, IssueComment>(
        "INSERT INTO issue_comments (issue_id, author_id, body) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(issue_id)
    .bind(author_id)
    .bind(body.trim())
    .fetch_one(pool)
    .await?;

    sqlx::query("UPDATE issues SET updated_at = datetime('now') WHERE id = ?")
        .bind(issue_id)
        .execute(pool)
        .await?;

    Ok(comment)
}

/// Cross-pool variant: comments from tenant pool, authors from control pool.
pub async fn list_issue_comments_cross(
    control_pool: &SqlitePool,
    tenant_pool: &SqlitePool,
    issue_id: i64,
) -> Result<Vec<(IssueComment, User)>> {
    let comments = sqlx::query_as::<_, IssueComment>(
        "SELECT * FROM issue_comments WHERE issue_id = ? ORDER BY created_at ASC",
    )
    .bind(issue_id)
    .fetch_all(tenant_pool)
    .await?;

    let mut result = Vec::new();
    for comment in comments {
        let author = get_user_by_id(control_pool, comment.author_id).await?;
        result.push((comment, author));
    }
    Ok(result)
}

// --- AI Commit Metadata queries ---

#[allow(clippy::too_many_arguments)]
pub async fn insert_ai_metadata(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
    ai_tool: &str,
    ai_model: Option<&str>,
    ai_prompt: Option<&str>,
    ai_session_id: Option<&str>,
    ai_files_touched: Option<&str>,
    ai_prompt_index: Option<i64>,
) -> Result<AiCommitMetadata> {
    let meta = sqlx::query_as::<_, AiCommitMetadata>(
        "INSERT OR REPLACE INTO ai_commit_metadata (repo_id, commit_sha, ai_tool, ai_model, ai_prompt, ai_session_id, ai_files_touched, ai_prompt_index) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .bind(ai_tool)
    .bind(ai_model)
    .bind(ai_prompt)
    .bind(ai_session_id)
    .bind(ai_files_touched)
    .bind(ai_prompt_index)
    .fetch_one(pool)
    .await?;
    Ok(meta)
}

pub async fn get_ai_metadata_for_commit(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
) -> Result<Option<AiCommitMetadata>> {
    let meta = sqlx::query_as::<_, AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? AND commit_sha = ?",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .fetch_optional(pool)
    .await?;
    Ok(meta)
}

/// Batch-fetch AI metadata for multiple commit SHAs.
pub async fn get_ai_metadata_for_commits(
    pool: &SqlitePool,
    repo_id: i64,
    shas: &[String],
) -> Result<Vec<AiCommitMetadata>> {
    if shas.is_empty() {
        return Ok(vec![]);
    }
    // Build dynamic IN clause since sqlx doesn't support array binding for SQLite
    let placeholders: Vec<&str> = shas.iter().map(|_| "?").collect();
    let sql = format!(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? AND commit_sha IN ({})",
        placeholders.join(", ")
    );
    let mut query = sqlx::query_as::<_, AiCommitMetadata>(&sql).bind(repo_id);
    for sha in shas {
        query = query.bind(sha);
    }
    let result = query.fetch_all(pool).await?;
    Ok(result)
}

/// List distinct AI sessions for a repository with summary info.
pub async fn list_ai_sessions(pool: &SqlitePool, repo_id: i64) -> Result<Vec<AiCommitMetadata>> {
    let metas = sqlx::query_as::<_, AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? ORDER BY created_at DESC",
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;
    Ok(metas)
}

/// Get AI commits without a session ID.
pub async fn get_unsessioned_ai_commits(
    pool: &SqlitePool,
    repo_id: i64,
    query: &str,
) -> Result<Vec<AiCommitMetadata>> {
    let metas = if query.is_empty() {
        sqlx::query_as::<_, AiCommitMetadata>(
            "SELECT * FROM ai_commit_metadata WHERE repo_id = ? AND ai_session_id IS NULL ORDER BY created_at DESC",
        )
        .bind(repo_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, AiCommitMetadata>(
            "SELECT * FROM ai_commit_metadata WHERE repo_id = ? AND ai_session_id IS NULL \
             AND ai_prompt LIKE '%' || ? || '%' ORDER BY created_at DESC",
        )
        .bind(repo_id)
        .bind(query)
        .fetch_all(pool)
        .await?
    };
    Ok(metas)
}

/// Get all AI metadata for a specific session.
pub async fn get_ai_metadata_by_session(
    pool: &SqlitePool,
    repo_id: i64,
    session_id: &str,
) -> Result<Vec<AiCommitMetadata>> {
    let metas = sqlx::query_as::<_, AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? AND ai_session_id = ? ORDER BY created_at DESC",
    )
    .bind(repo_id)
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(metas)
}

// --- AI Diff Summary queries ---

pub async fn get_diff_summary(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
) -> Result<Option<AiDiffSummary>> {
    let summary = sqlx::query_as::<_, AiDiffSummary>(
        "SELECT * FROM ai_diff_summaries WHERE repo_id = ? AND commit_sha = ?",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .fetch_optional(pool)
    .await?;
    Ok(summary)
}

pub async fn upsert_diff_summary(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
    summary: &str,
    risk_flags: Option<&str>,
    generated_by: &str,
) -> Result<AiDiffSummary> {
    let row = sqlx::query_as::<_, AiDiffSummary>(
        "INSERT OR REPLACE INTO ai_diff_summaries (repo_id, commit_sha, summary, risk_flags, generated_by) \
         VALUES (?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .bind(summary)
    .bind(risk_flags)
    .bind(generated_by)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Update the has_remix flag for a repository.
pub async fn update_has_remix(pool: &SqlitePool, repo_id: i64, has_remix: bool) -> Result<()> {
    sqlx::query("UPDATE repositories SET has_remix = ? WHERE id = ?")
        .bind(has_remix)
        .bind(repo_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Update the visibility (is_private) flag for a repository.
pub async fn update_repository_visibility(
    pool: &SqlitePool,
    repo_id: i64,
    is_private: bool,
) -> Result<()> {
    sqlx::query(
        "UPDATE repositories SET is_private = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(is_private)
    .bind(repo_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Search public repositories that have REMIX.md (remixable).
pub async fn search_remixable_repositories(
    pool: &SqlitePool,
    query: &str,
) -> Result<Vec<(User, Repository)>> {
    let rows = if query.is_empty() {
        sqlx::query_as::<_, (i64, String, String, String, String, i64, String, String, bool, Option<i64>, bool, String, String)>(
            "SELECT u.id, u.username, u.email, u.password_hash, u.display_name, r.id, r.name, r.description, r.is_private, r.forked_from, r.has_remix, r.created_at, r.updated_at \
             FROM repositories r JOIN users u ON r.owner_id = u.id \
             WHERE r.is_private = 0 AND r.has_remix = 1 \
             ORDER BY r.updated_at DESC LIMIT 50",
        )
        .fetch_all(pool)
        .await?
    } else {
        let pattern = format!("%{}%", query);
        sqlx::query_as::<_, (i64, String, String, String, String, i64, String, String, bool, Option<i64>, bool, String, String)>(
            "SELECT u.id, u.username, u.email, u.password_hash, u.display_name, r.id, r.name, r.description, r.is_private, r.forked_from, r.has_remix, r.created_at, r.updated_at \
             FROM repositories r JOIN users u ON r.owner_id = u.id \
             WHERE r.is_private = 0 AND r.has_remix = 1 AND (r.name LIKE ? OR u.username LIKE ? OR r.description LIKE ?) \
             ORDER BY r.updated_at DESC LIMIT 50",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                uid,
                username,
                email,
                pw,
                display_name,
                rid,
                name,
                desc,
                private,
                forked_from,
                has_remix,
                created,
                updated,
            )| {
                (
                    User {
                        id: uid,
                        username,
                        email,
                        password_hash: pw,
                        display_name,
                        is_admin: false,
                        is_disabled: false,
                        created_at: created.clone(),
                        updated_at: updated.clone(),
                    },
                    Repository {
                        id: rid,
                        owner_id: uid,
                        name,
                        description: desc,
                        is_private: private,
                        forked_from,
                        has_remix,
                        created_at: created,
                        updated_at: updated,
                    },
                )
            },
        )
        .collect())
}

/// Session summary row from GROUP BY query.
pub struct SessionSummaryRow {
    pub session_id: String,
    pub ai_tool: String,
    pub first_time: String,
    pub last_time: String,
    pub commit_count: i64,
    pub first_prompt: Option<String>,
}

/// List session summaries for a repo (efficient GROUP BY query).
pub async fn list_session_summaries(
    pool: &SqlitePool,
    repo_id: i64,
    query: &str,
) -> Result<Vec<SessionSummaryRow>> {
    let rows: Vec<(String, String, String, String, i64, Option<String>)> = if query.is_empty() {
        sqlx::query_as(
            "SELECT ai_session_id, ai_tool, MIN(created_at), MAX(created_at), COUNT(*), MIN(ai_prompt) \
             FROM ai_commit_metadata \
             WHERE repo_id = ? AND ai_session_id IS NOT NULL \
             GROUP BY ai_session_id \
             ORDER BY MAX(created_at) DESC",
        )
        .bind(repo_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT ai_session_id, ai_tool, MIN(created_at), MAX(created_at), COUNT(*), MIN(ai_prompt) \
             FROM ai_commit_metadata \
             WHERE repo_id = ? AND ai_session_id IS NOT NULL \
             AND ai_session_id IN ( \
                 SELECT ai_session_id FROM ai_commit_metadata \
                 WHERE repo_id = ? AND ai_prompt LIKE '%' || ? || '%' \
             ) \
             GROUP BY ai_session_id \
             ORDER BY MAX(created_at) DESC",
        )
        .bind(repo_id)
        .bind(repo_id)
        .bind(query)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(session_id, ai_tool, first_time, last_time, commit_count, first_prompt)| {
                SessionSummaryRow {
                    session_id,
                    ai_tool,
                    first_time,
                    last_time,
                    commit_count,
                    first_prompt,
                }
            },
        )
        .collect())
}

/// Prompt group row from GROUP BY query — groups commits by prompt within sessions.
pub struct PromptGroupRow {
    pub ai_session_id: Option<String>,
    pub ai_prompt_index: Option<i64>,
    pub ai_prompt: Option<String>,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub commit_count: i64,
    pub first_time: String,
    pub last_time: String,
    pub commit_shas: Vec<String>,
}

/// List prompt groups for a repo: each row represents one developer prompt and the commits it produced.
/// Groups by (ai_session_id, ai_prompt_index), falling back to (ai_session_id, ai_prompt) for legacy data.
#[allow(clippy::type_complexity)]
pub async fn list_prompt_groups(
    pool: &SqlitePool,
    repo_id: i64,
    query: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<PromptGroupRow>> {
    // We use a two-level grouping key: session_id + COALESCE(prompt_index, prompt text, commit_sha)
    // This ensures: with prompt_index → group by index; without → group by prompt text; solo → own group
    let group_key = "ai_session_id, COALESCE(CAST(ai_prompt_index AS TEXT), ai_prompt, commit_sha)";

    let (sql, needs_query_bind) = if query.is_empty() {
        (
            format!(
                "SELECT ai_session_id, ai_prompt_index, MIN(ai_prompt), ai_tool, MIN(ai_model), \
             COUNT(*), MIN(created_at), MAX(created_at), GROUP_CONCAT(commit_sha, ',') \
             FROM ai_commit_metadata \
             WHERE repo_id = ? \
             GROUP BY {} \
             ORDER BY MAX(created_at) DESC \
             LIMIT ? OFFSET ?",
                group_key
            ),
            false,
        )
    } else {
        (
            format!(
                "SELECT ai_session_id, ai_prompt_index, MIN(ai_prompt), ai_tool, MIN(ai_model), \
             COUNT(*), MIN(created_at), MAX(created_at), GROUP_CONCAT(commit_sha, ',') \
             FROM ai_commit_metadata \
             WHERE repo_id = ? AND ai_prompt LIKE '%' || ? || '%' \
             GROUP BY {} \
             ORDER BY MAX(created_at) DESC \
             LIMIT ? OFFSET ?",
                group_key
            ),
            true,
        )
    };

    let rows: Vec<(
        Option<String>,
        Option<i64>,
        Option<String>,
        String,
        Option<String>,
        i64,
        String,
        String,
        String,
    )> = {
        let mut q = sqlx::query_as(&sql).bind(repo_id);
        if needs_query_bind {
            q = q.bind(query);
        }
        q = q.bind(limit).bind(offset);
        q.fetch_all(pool).await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(session_id, prompt_index, prompt, tool, model, count, first, last, shas_csv)| {
                PromptGroupRow {
                    ai_session_id: session_id,
                    ai_prompt_index: prompt_index,
                    ai_prompt: prompt,
                    ai_tool: tool,
                    ai_model: model,
                    commit_count: count,
                    first_time: first,
                    last_time: last,
                    commit_shas: shas_csv.split(',').map(|s| s.to_string()).collect(),
                }
            },
        )
        .collect())
}

/// Batch-fetch AI metadata as a HashMap keyed by commit SHA for O(1) lookup.
pub async fn get_ai_metadata_for_commits_map(
    pool: &SqlitePool,
    repo_id: i64,
    shas: &[String],
) -> Result<std::collections::HashMap<String, AiCommitMetadata>> {
    let metas = get_ai_metadata_for_commits(pool, repo_id, shas).await?;
    Ok(metas
        .into_iter()
        .map(|m| (m.commit_sha.clone(), m))
        .collect())
}

/// Get all commits for a specific prompt within a session.
pub async fn get_commits_for_prompt_group(
    pool: &SqlitePool,
    repo_id: i64,
    session_id: &str,
    prompt_index: i64,
) -> Result<Vec<AiCommitMetadata>> {
    let metas = sqlx::query_as::<_, AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata \
         WHERE repo_id = ? AND ai_session_id = ? AND ai_prompt_index = ? \
         ORDER BY created_at ASC",
    )
    .bind(repo_id)
    .bind(session_id)
    .bind(prompt_index)
    .fetch_all(pool)
    .await?;
    Ok(metas)
}

/// Count total prompt groups for pagination.
pub async fn count_prompt_groups(pool: &SqlitePool, repo_id: i64, query: &str) -> Result<i64> {
    let group_key = "ai_session_id, COALESCE(CAST(ai_prompt_index AS TEXT), ai_prompt, commit_sha)";
    let count: (i64,) = if query.is_empty() {
        sqlx::query_as(&format!(
            "SELECT COUNT(*) FROM (SELECT 1 FROM ai_commit_metadata WHERE repo_id = ? GROUP BY {})",
            group_key
        ))
        .bind(repo_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as(&format!(
            "SELECT COUNT(*) FROM (SELECT 1 FROM ai_commit_metadata WHERE repo_id = ? AND ai_prompt LIKE '%' || ? || '%' GROUP BY {})",
            group_key
        ))
        .bind(repo_id)
        .bind(query)
        .fetch_one(pool)
        .await?
    };
    Ok(count.0)
}

// --- User Settings queries ---

pub async fn get_user_settings(pool: &SqlitePool, user_id: i64) -> Result<Option<UserSettings>> {
    let settings =
        sqlx::query_as::<_, UserSettings>("SELECT * FROM user_settings WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(settings)
}

pub async fn upsert_user_settings(
    pool: &SqlitePool,
    user_id: i64,
    llm_provider: Option<&str>,
    llm_api_key: Option<&str>,
    llm_model: Option<&str>,
    llm_base_url: Option<&str>,
    master_key: &[u8],
) -> Result<UserSettings> {
    let encrypted_key = llm_api_key
        .map(|k| crate::crypto::encrypt_secret(master_key, k))
        .transpose()?;

    let row = sqlx::query_as::<_, UserSettings>(
        "INSERT INTO user_settings (user_id, llm_provider, llm_api_key, llm_model, llm_base_url, updated_at) \
         VALUES (?, ?, ?, ?, ?, datetime('now')) \
         ON CONFLICT(user_id) DO UPDATE SET \
         llm_provider = excluded.llm_provider, \
         llm_api_key = excluded.llm_api_key, \
         llm_model = excluded.llm_model, \
         llm_base_url = excluded.llm_base_url, \
         updated_at = datetime('now') \
         RETURNING *",
    )
    .bind(user_id)
    .bind(llm_provider)
    .bind(encrypted_key.as_deref())
    .bind(llm_model)
    .bind(llm_base_url)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Migrate existing plaintext API keys to encrypted form. Called once at startup.
pub async fn migrate_encrypt_api_keys(pool: &SqlitePool, master_key: &[u8]) -> Result<()> {
    let rows = sqlx::query_as::<_, UserSettings>(
        "SELECT * FROM user_settings WHERE llm_api_key IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        if let Some(ref key) = row.llm_api_key
            && !crate::crypto::is_encrypted(key)
        {
            let encrypted = crate::crypto::encrypt_secret(master_key, key)?;
            sqlx::query("UPDATE user_settings SET llm_api_key = ? WHERE id = ?")
                .bind(&encrypted)
                .bind(row.id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

// --- Webhook queries ---

pub async fn create_webhook(
    pool: &SqlitePool,
    repo_id: i64,
    url: &str,
    secret: Option<&str>,
) -> Result<RepoWebhook> {
    let hook = sqlx::query_as::<_, RepoWebhook>(
        "INSERT INTO repo_webhooks (repo_id, url, secret) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(url)
    .bind(secret)
    .fetch_one(pool)
    .await?;
    Ok(hook)
}

pub async fn list_webhooks(pool: &SqlitePool, repo_id: i64) -> Result<Vec<RepoWebhook>> {
    let hooks = sqlx::query_as::<_, RepoWebhook>(
        "SELECT * FROM repo_webhooks WHERE repo_id = ? ORDER BY created_at DESC",
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;
    Ok(hooks)
}

pub async fn get_active_webhooks(pool: &SqlitePool, repo_id: i64) -> Result<Vec<RepoWebhook>> {
    let hooks = sqlx::query_as::<_, RepoWebhook>(
        "SELECT * FROM repo_webhooks WHERE repo_id = ? AND active = 1",
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;
    Ok(hooks)
}

pub async fn delete_webhook(pool: &SqlitePool, webhook_id: i64, repo_id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM repo_webhooks WHERE id = ? AND repo_id = ?")
        .bind(webhook_id)
        .bind(repo_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(OxigitError::NotFound("Webhook not found".into()));
    }
    Ok(())
}

// --- Deploy Preview queries ---

pub async fn create_deploy_preview(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
    branch: &str,
) -> Result<DeployPreview> {
    let preview = sqlx::query_as::<_, DeployPreview>(
        "INSERT OR REPLACE INTO deploy_previews (repo_id, commit_sha, branch) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .bind(branch)
    .fetch_one(pool)
    .await?;
    Ok(preview)
}

pub async fn update_deploy_preview(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
    preview_url: &str,
    status: &str,
) -> Result<DeployPreview> {
    let preview = sqlx::query_as::<_, DeployPreview>(
        "UPDATE deploy_previews SET preview_url = ?, status = ?, updated_at = datetime('now') \
         WHERE repo_id = ? AND commit_sha = ? RETURNING *",
    )
    .bind(preview_url)
    .bind(status)
    .bind(repo_id)
    .bind(commit_sha)
    .fetch_one(pool)
    .await?;
    Ok(preview)
}

pub async fn get_deploy_preview(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
) -> Result<Option<DeployPreview>> {
    let preview = sqlx::query_as::<_, DeployPreview>(
        "SELECT * FROM deploy_previews WHERE repo_id = ? AND commit_sha = ?",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .fetch_optional(pool)
    .await?;
    Ok(preview)
}

// --- Dashboard aggregation queries ---

pub struct UserAiStats {
    pub total_ai_commits: i64,
    pub total_sessions: i64,
    pub total_repos_with_ai: i64,
}

pub async fn get_user_ai_stats(pool: &SqlitePool, user_id: i64) -> Result<UserAiStats> {
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(DISTINCT m.ai_session_id), COUNT(DISTINCT r.id) \
         FROM ai_commit_metadata m JOIN repositories r ON m.repo_id = r.id \
         WHERE r.owner_id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(UserAiStats {
        total_ai_commits: row.0,
        total_sessions: row.1,
        total_repos_with_ai: row.2,
    })
}

pub struct ToolUsageRow {
    pub ai_tool: String,
    pub commit_count: i64,
}

pub async fn get_user_tool_usage(pool: &SqlitePool, user_id: i64) -> Result<Vec<ToolUsageRow>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT m.ai_tool, COUNT(*) \
         FROM ai_commit_metadata m JOIN repositories r ON m.repo_id = r.id \
         WHERE r.owner_id = ? GROUP BY m.ai_tool ORDER BY COUNT(*) DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(ai_tool, commit_count)| ToolUsageRow {
            ai_tool,
            commit_count,
        })
        .collect())
}

pub struct RecentSessionRow {
    pub session_id: String,
    pub ai_tool: String,
    pub repo_name: String,
    pub commit_count: i64,
    pub last_time: String,
    pub first_prompt: Option<String>,
}

pub async fn get_user_recent_sessions(
    pool: &SqlitePool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<RecentSessionRow>> {
    let rows: Vec<(String, String, String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT m.ai_session_id, m.ai_tool, r.name, COUNT(*), MAX(m.created_at), MIN(m.ai_prompt) \
         FROM ai_commit_metadata m JOIN repositories r ON m.repo_id = r.id \
         WHERE r.owner_id = ? AND m.ai_session_id IS NOT NULL \
         GROUP BY m.ai_session_id ORDER BY MAX(m.created_at) DESC LIMIT ?",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(session_id, ai_tool, repo_name, commit_count, last_time, first_prompt)| {
                RecentSessionRow {
                    session_id,
                    ai_tool,
                    repo_name,
                    commit_count,
                    last_time,
                    first_prompt,
                }
            },
        )
        .collect())
}

pub async fn get_user_risk_summaries(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<AiDiffSummary>> {
    let rows = sqlx::query_as::<_, AiDiffSummary>(
        "SELECT d.* FROM ai_diff_summaries d JOIN repositories r ON d.repo_id = r.id \
         WHERE r.owner_id = ? AND d.risk_flags IS NOT NULL AND d.risk_flags != '[]'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// --- Vibe Score session data queries ---

/// Session data row for vibe score computation.
pub struct SessionDataRow {
    pub session_id: String,
    pub ai_tool: String,
    pub commit_count: i64,
    pub prompt_count: i64,
    pub first_time: String,
    pub last_time: String,
    pub commit_shas: Vec<String>,
    pub first_prompt: Option<String>,
}

/// Get session data for all sessions in a repo (for vibe scoring).
#[allow(clippy::type_complexity)]
pub async fn get_repo_session_data(pool: &SqlitePool, repo_id: i64) -> Result<Vec<SessionDataRow>> {
    let group_key = "COALESCE(CAST(ai_prompt_index AS TEXT), ai_prompt, commit_sha)";
    let rows: Vec<(
        String,
        String,
        i64,
        i64,
        String,
        String,
        String,
        Option<String>,
    )> = sqlx::query_as(&format!(
        "SELECT ai_session_id, ai_tool, COUNT(*), \
         COUNT(DISTINCT {}), \
         MIN(created_at), MAX(created_at), GROUP_CONCAT(commit_sha, ','), MIN(ai_prompt) \
         FROM ai_commit_metadata \
         WHERE repo_id = ? AND ai_session_id IS NOT NULL \
         GROUP BY ai_session_id \
         ORDER BY MAX(created_at) DESC \
         LIMIT 50",
        group_key
    ))
    .bind(repo_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(sid, tool, cc, pc, ft, lt, shas, fp)| SessionDataRow {
            session_id: sid,
            ai_tool: tool,
            commit_count: cc,
            prompt_count: pc,
            first_time: ft,
            last_time: lt,
            commit_shas: shas.split(',').map(|s| s.to_string()).collect(),
            first_prompt: fp,
        })
        .collect())
}

/// Get session data across all repos owned by a user (for user-level vibe scoring).
#[allow(clippy::type_complexity)]
pub async fn get_user_session_data(pool: &SqlitePool, user_id: i64) -> Result<Vec<SessionDataRow>> {
    let group_key = "COALESCE(CAST(m.ai_prompt_index AS TEXT), m.ai_prompt, m.commit_sha)";
    let rows: Vec<(
        String,
        String,
        i64,
        i64,
        String,
        String,
        String,
        Option<String>,
    )> = sqlx::query_as(&format!(
        "SELECT m.ai_session_id, m.ai_tool, COUNT(*), \
         COUNT(DISTINCT {}), \
         MIN(m.created_at), MAX(m.created_at), GROUP_CONCAT(m.commit_sha, ','), MIN(m.ai_prompt) \
         FROM ai_commit_metadata m JOIN repositories r ON m.repo_id = r.id \
         WHERE r.owner_id = ? AND m.ai_session_id IS NOT NULL \
         GROUP BY m.ai_session_id \
         ORDER BY MAX(m.created_at) DESC \
         LIMIT 50",
        group_key
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(sid, tool, cc, pc, ft, lt, shas, fp)| SessionDataRow {
            session_id: sid,
            ai_tool: tool,
            commit_count: cc,
            prompt_count: pc,
            first_time: ft,
            last_time: lt,
            commit_shas: shas.split(',').map(|s| s.to_string()).collect(),
            first_prompt: fp,
        })
        .collect())
}

// --- Merge Conflict Resolution queries ---

#[allow(clippy::too_many_arguments)]
pub async fn create_merge_conflict(
    pool: &SqlitePool,
    repo_id: i64,
    user_id: i64,
    operation_type: &str,
    target_ref: &str,
    source_ref: &str,
    merge_base: &str,
    auto_tree: Option<&str>,
    context_json: Option<&str>,
) -> Result<MergeConflict> {
    let row = sqlx::query_as::<_, MergeConflict>(
        "INSERT INTO merge_conflicts (repo_id, user_id, operation_type, target_ref, source_ref, merge_base, auto_tree, context_json) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(user_id)
    .bind(operation_type)
    .bind(target_ref)
    .bind(source_ref)
    .bind(merge_base)
    .bind(auto_tree)
    .bind(context_json)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn create_conflict_file(
    pool: &SqlitePool,
    conflict_id: i64,
    file_path: &str,
    conflict_type: &str,
) -> Result<MergeConflictFile> {
    let row = sqlx::query_as::<_, MergeConflictFile>(
        "INSERT INTO merge_conflict_files (merge_conflict_id, file_path, conflict_type) \
         VALUES (?, ?, ?) RETURNING *",
    )
    .bind(conflict_id)
    .bind(file_path)
    .bind(conflict_type)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_merge_conflict(pool: &SqlitePool, id: i64) -> Result<Option<MergeConflict>> {
    let row = sqlx::query_as::<_, MergeConflict>("SELECT * FROM merge_conflicts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_conflict_files(
    pool: &SqlitePool,
    conflict_id: i64,
) -> Result<Vec<MergeConflictFile>> {
    let rows = sqlx::query_as::<_, MergeConflictFile>(
        "SELECT * FROM merge_conflict_files WHERE merge_conflict_id = ? ORDER BY file_path",
    )
    .bind(conflict_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn resolve_conflict_file(
    pool: &SqlitePool,
    file_id: i64,
    resolution: &str,
    resolved_content: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE merge_conflict_files SET resolution = ?, resolved_content = ?, resolved_at = datetime('now') WHERE id = ?",
    )
    .bind(resolution)
    .bind(resolved_content)
    .bind(file_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn complete_merge_conflict(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE merge_conflicts SET status = 'resolved', updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn cancel_merge_conflict(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE merge_conflicts SET status = 'cancelled', updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

// --- Guardrail queries ---

pub async fn upsert_guardrail_rule(
    pool: &SqlitePool,
    repo_id: i64,
    category: &str,
    action: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO guardrail_rules (repo_id, category, action) VALUES (?, ?, ?) \
         ON CONFLICT(repo_id, category) DO UPDATE SET action = excluded.action, updated_at = datetime('now')",
    )
    .bind(repo_id)
    .bind(category)
    .bind(action)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_guardrail_rules(pool: &SqlitePool, repo_id: i64) -> Result<Vec<GuardrailRule>> {
    let rules =
        sqlx::query_as::<_, GuardrailRule>("SELECT * FROM guardrail_rules WHERE repo_id = ?")
            .bind(repo_id)
            .fetch_all(pool)
            .await?;
    Ok(rules)
}

pub async fn upsert_guardrail_config(
    pool: &SqlitePool,
    repo_id: i64,
    min_vibe_score: Option<i64>,
    max_files_per_push: Option<i64>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO guardrail_config (repo_id, min_vibe_score, max_files_per_push) VALUES (?, ?, ?) \
         ON CONFLICT(repo_id) DO UPDATE SET min_vibe_score = excluded.min_vibe_score, \
         max_files_per_push = excluded.max_files_per_push, updated_at = datetime('now')",
    )
    .bind(repo_id)
    .bind(min_vibe_score)
    .bind(max_files_per_push)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_guardrail_config(
    pool: &SqlitePool,
    repo_id: i64,
) -> Result<Option<GuardrailConfig>> {
    let config =
        sqlx::query_as::<_, GuardrailConfig>("SELECT * FROM guardrail_config WHERE repo_id = ?")
            .bind(repo_id)
            .fetch_optional(pool)
            .await?;
    Ok(config)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_guardrail_violation(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
    ref_name: Option<&str>,
    rule_category: &str,
    action_taken: &str,
    severity: &str,
    message: &str,
    file_path: Option<&str>,
    pushed_by: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO guardrail_violations \
         (repo_id, commit_sha, ref_name, rule_category, action_taken, severity, message, file_path, pushed_by) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .bind(ref_name)
    .bind(rule_category)
    .bind(action_taken)
    .bind(severity)
    .bind(message)
    .bind(file_path)
    .bind(pushed_by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_guardrail_violations(
    pool: &SqlitePool,
    repo_id: i64,
    limit: i64,
) -> Result<Vec<GuardrailViolation>> {
    let rows = sqlx::query_as::<_, GuardrailViolation>(
        "SELECT * FROM guardrail_violations WHERE repo_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(repo_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// --- Recipe queries ---

#[allow(clippy::too_many_arguments)]
pub async fn create_recipe(
    pool: &SqlitePool,
    repo_id: i64,
    session_id: &str,
    author_id: i64,
    title: &str,
    description: &str,
    ai_tool: &str,
    ai_model: Option<&str>,
    tags: Option<&str>,
    prompt_count: i64,
    file_count: i64,
    vibe_score: Option<i64>,
) -> Result<Recipe> {
    let recipe = sqlx::query_as::<_, Recipe>(
        "INSERT INTO recipes (repo_id, session_id, author_id, title, description, ai_tool, ai_model, tags, prompt_count, file_count, vibe_score) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id).bind(session_id).bind(author_id)
    .bind(title).bind(description).bind(ai_tool).bind(ai_model)
    .bind(tags).bind(prompt_count).bind(file_count).bind(vibe_score)
    .fetch_one(pool)
    .await?;
    Ok(recipe)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_recipe_step(
    pool: &SqlitePool,
    recipe_id: i64,
    step_order: i64,
    prompt_text: Option<&str>,
    prompt_index: Option<i64>,
    commit_message: &str,
    files_json: Option<&str>,
    diff_text: Option<&str>,
) -> Result<RecipeStep> {
    let step = sqlx::query_as::<_, RecipeStep>(
        "INSERT INTO recipe_steps (recipe_id, step_order, prompt_text, prompt_index, commit_message, files_json, diff_text) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(recipe_id).bind(step_order).bind(prompt_text).bind(prompt_index)
    .bind(commit_message).bind(files_json).bind(diff_text)
    .fetch_one(pool)
    .await?;
    Ok(step)
}

pub async fn get_recipe_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Recipe>> {
    let recipe = sqlx::query_as::<_, Recipe>("SELECT * FROM recipes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(recipe)
}

pub async fn get_recipe_steps(pool: &SqlitePool, recipe_id: i64) -> Result<Vec<RecipeStep>> {
    let steps = sqlx::query_as::<_, RecipeStep>(
        "SELECT * FROM recipe_steps WHERE recipe_id = ? ORDER BY step_order ASC",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await?;
    Ok(steps)
}

pub async fn get_recipe_for_session(
    pool: &SqlitePool,
    repo_id: i64,
    session_id: &str,
) -> Result<Option<Recipe>> {
    let recipe =
        sqlx::query_as::<_, Recipe>("SELECT * FROM recipes WHERE repo_id = ? AND session_id = ?")
            .bind(repo_id)
            .bind(session_id)
            .fetch_optional(pool)
            .await?;
    Ok(recipe)
}

/// Recipe with joined author/repo info for listing.
pub struct RecipeWithContext {
    pub recipe: Recipe,
    pub author_username: String,
    pub repo_name: String,
    pub repo_owner: String,
}

pub async fn search_public_recipes(
    pool: &SqlitePool,
    query: &str,
    sort: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<RecipeWithContext>> {
    let order_clause = match sort {
        "top_vibe" => "r.vibe_score DESC NULLS LAST",
        "most_replayed" => "r.replay_count DESC",
        _ => "r.created_at DESC",
    };

    // Fetch recipes, then enrich with author/repo info
    let recipes: Vec<Recipe> = if query.is_empty() {
        sqlx::query_as(&format!(
            "SELECT r.* FROM recipes r JOIN repositories rep ON r.repo_id = rep.id \
             WHERE r.is_public = 1 AND rep.is_private = 0 \
             ORDER BY {} LIMIT ? OFFSET ?",
            order_clause
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(&format!(
            "SELECT r.* FROM recipes r JOIN repositories rep ON r.repo_id = rep.id \
             WHERE r.is_public = 1 AND rep.is_private = 0 \
             AND (r.title LIKE '%' || ? || '%' OR r.description LIKE '%' || ? || '%' OR r.tags LIKE '%' || ? || '%') \
             ORDER BY {} LIMIT ? OFFSET ?",
            order_clause
        ))
        .bind(query).bind(query).bind(query)
        .bind(limit).bind(offset).fetch_all(pool).await?
    };

    let mut results = Vec::new();
    for recipe in recipes {
        let author: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = ?")
            .bind(recipe.author_id)
            .fetch_optional(pool)
            .await?;
        let repo_info: Option<(String, i64)> =
            sqlx::query_as("SELECT name, owner_id FROM repositories WHERE id = ?")
                .bind(recipe.repo_id)
                .fetch_optional(pool)
                .await?;
        let (repo_name, owner_id) = repo_info.unwrap_or(("unknown".into(), 0));
        let owner: Option<(String,)> = sqlx::query_as("SELECT username FROM users WHERE id = ?")
            .bind(owner_id)
            .fetch_optional(pool)
            .await?;

        results.push(RecipeWithContext {
            author_username: author.map(|a| a.0).unwrap_or("unknown".into()),
            repo_name,
            repo_owner: owner.map(|o| o.0).unwrap_or("unknown".into()),
            recipe,
        });
    }
    Ok(results)
}

pub async fn count_public_recipes(pool: &SqlitePool, query: &str) -> Result<i64> {
    let count: (i64,) = if query.is_empty() {
        sqlx::query_as(
            "SELECT COUNT(*) FROM recipes r JOIN repositories rep ON r.repo_id = rep.id \
             WHERE r.is_public = 1 AND rep.is_private = 0",
        )
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT COUNT(*) FROM recipes r JOIN repositories rep ON r.repo_id = rep.id \
             WHERE r.is_public = 1 AND rep.is_private = 0 \
             AND (r.title LIKE '%' || ? || '%' OR r.description LIKE '%' || ? || '%' OR r.tags LIKE '%' || ? || '%')",
        ).bind(query).bind(query).bind(query).fetch_one(pool).await?
    };
    Ok(count.0)
}

pub async fn increment_replay_count(pool: &SqlitePool, recipe_id: i64) -> Result<()> {
    sqlx::query("UPDATE recipes SET replay_count = replay_count + 1 WHERE id = ?")
        .bind(recipe_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_recipe_replay(
    pool: &SqlitePool,
    recipe_id: i64,
    user_id: i64,
    target_repo_id: i64,
    target_branch: &str,
    mode: &str,
    status: &str,
    steps_applied: i64,
    error_message: Option<&str>,
) -> Result<RecipeReplay> {
    let replay = sqlx::query_as::<_, RecipeReplay>(
        "INSERT INTO recipe_replays (recipe_id, user_id, target_repo_id, target_branch, mode, status, steps_applied, error_message) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(recipe_id).bind(user_id).bind(target_repo_id).bind(target_branch)
    .bind(mode).bind(status).bind(steps_applied).bind(error_message)
    .fetch_one(pool)
    .await?;
    Ok(replay)
}

// --- Subscription queries ---

#[cfg(feature = "saas")]
pub async fn get_subscription(pool: &SqlitePool, user_id: i64) -> Result<Option<Subscription>> {
    let sub = sqlx::query_as::<_, Subscription>("SELECT * FROM subscriptions WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(sub)
}

#[cfg(feature = "saas")]
pub async fn get_subscription_by_stripe_customer(
    pool: &SqlitePool,
    stripe_customer_id: &str,
) -> Result<Option<Subscription>> {
    let sub = sqlx::query_as::<_, Subscription>(
        "SELECT * FROM subscriptions WHERE stripe_customer_id = ?",
    )
    .bind(stripe_customer_id)
    .fetch_optional(pool)
    .await?;
    Ok(sub)
}

#[cfg(feature = "saas")]
pub async fn get_subscription_by_stripe_subscription(
    pool: &SqlitePool,
    stripe_subscription_id: &str,
) -> Result<Option<Subscription>> {
    let sub = sqlx::query_as::<_, Subscription>(
        "SELECT * FROM subscriptions WHERE stripe_subscription_id = ?",
    )
    .bind(stripe_subscription_id)
    .fetch_optional(pool)
    .await?;
    Ok(sub)
}

#[cfg(feature = "saas")]
#[allow(clippy::too_many_arguments)]
pub async fn upsert_subscription(
    pool: &SqlitePool,
    user_id: i64,
    stripe_customer_id: &str,
    stripe_subscription_id: Option<&str>,
    plan: &str,
    status: &str,
    current_period_end: Option<&str>,
    seats: i64,
) -> Result<Subscription> {
    let mut tx = pool.begin().await?;

    // Remove any stale subscription holding this stripe_customer_id for a
    // different user, so the INSERT below won't violate the unique index on
    // stripe_customer_id (the ON CONFLICT clause only covers user_id).
    sqlx::query("DELETE FROM subscriptions WHERE stripe_customer_id = ? AND user_id != ?")
        .bind(stripe_customer_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    let sub = sqlx::query_as::<_, Subscription>(
        "INSERT INTO subscriptions (user_id, stripe_customer_id, stripe_subscription_id, plan, status, current_period_end, seats) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(user_id) DO UPDATE SET \
           stripe_customer_id = excluded.stripe_customer_id, \
           stripe_subscription_id = excluded.stripe_subscription_id, \
           plan = excluded.plan, \
           status = excluded.status, \
           current_period_end = excluded.current_period_end, \
           seats = excluded.seats, \
           updated_at = datetime('now') \
         RETURNING *",
    )
    .bind(user_id).bind(stripe_customer_id).bind(stripe_subscription_id)
    .bind(plan).bind(status).bind(current_period_end).bind(seats)
    .fetch_one(&mut *tx).await?;

    tx.commit().await?;
    Ok(sub)
}

#[cfg(feature = "saas")]
pub async fn update_subscription_status(
    pool: &SqlitePool,
    stripe_subscription_id: &str,
    status: &str,
    current_period_end: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE subscriptions SET status = ?, current_period_end = ?, updated_at = datetime('now') \
         WHERE stripe_subscription_id = ?",
    )
    .bind(status)
    .bind(current_period_end)
    .bind(stripe_subscription_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(feature = "saas")]
pub async fn cancel_subscription(pool: &SqlitePool, stripe_subscription_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE subscriptions SET status = 'canceled', updated_at = datetime('now') WHERE stripe_subscription_id = ?",
    )
    .bind(stripe_subscription_id).execute(pool).await?;
    Ok(())
}

#[allow(clippy::needless_return)]
pub async fn get_user_plan(pool: &SqlitePool, user_id: i64) -> Result<String> {
    #[cfg(feature = "saas")]
    {
        let sub = get_subscription(pool, user_id).await?;
        return match sub {
            Some(s) if s.status == "active" => Ok(s.plan),
            _ => Ok("free".to_string()),
        };
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = (pool, user_id);
        Ok("flat".to_string())
    }
}

// --- Founding member queries ---

#[cfg(feature = "saas")]
pub async fn count_founding_members(pool: &SqlitePool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM founding_members")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Atomically claim a founding member slot. Returns the slot number if successful,
/// or None if all 100 slots are taken.
#[cfg(feature = "saas")]
pub async fn claim_founding_slot(pool: &SqlitePool, user_id: i64) -> Result<Option<i64>> {
    // Atomic insert: only succeeds if fewer than 100 slots claimed and user hasn't claimed one
    let result = sqlx::query_as::<_, FoundingMember>(
        "INSERT INTO founding_members (user_id, slot_number) \
         SELECT ?, COALESCE((SELECT MAX(slot_number) FROM founding_members), 0) + 1 \
         WHERE (SELECT COUNT(*) FROM founding_members) < 100 \
         RETURNING *",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(result.map(|fm| fm.slot_number))
}

#[cfg(feature = "saas")]
pub async fn is_founding_member(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM founding_members WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

#[cfg(feature = "saas")]
pub async fn get_founding_member_slot(pool: &SqlitePool, user_id: i64) -> Result<Option<i64>> {
    let result: Option<(i64,)> =
        sqlx::query_as("SELECT slot_number FROM founding_members WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(result.map(|(n,)| n))
}

#[cfg(feature = "saas")]
pub async fn get_founding_member_slot_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<i64>> {
    let result: Option<(i64,)> = sqlx::query_as(
        "SELECT fm.slot_number FROM founding_members fm \
         JOIN users u ON u.id = fm.user_id \
         WHERE u.username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(result.map(|(n,)| n))
}

// --- Admin queries ---

pub async fn is_user_admin(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let (val,): (bool,) = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(val)
}

pub async fn is_user_disabled(pool: &SqlitePool, user_id: i64) -> Result<bool> {
    let (val,): (bool,) = sqlx::query_as("SELECT is_disabled FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(val)
}

pub async fn set_user_admin(pool: &SqlitePool, user_id: i64, is_admin: bool) -> Result<()> {
    sqlx::query("UPDATE users SET is_admin = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(is_admin)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_user_disabled(pool: &SqlitePool, user_id: i64, disabled: bool) -> Result<()> {
    sqlx::query("UPDATE users SET is_disabled = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(disabled)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_all_users(pool: &SqlitePool) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(users)
}

pub async fn count_users(pool: &SqlitePool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn count_repositories(pool: &SqlitePool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM repositories")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn count_private_repositories(pool: &SqlitePool, owner_id: i64) -> Result<i64> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM repositories WHERE owner_id = ? AND is_private = 1")
            .bind(owner_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

/// Count private repositories for a user via the repository_index table (control DB).
/// Use this in multi-tenant mode where repos table is in tenant DBs.
#[cfg(feature = "saas")]
pub async fn count_private_repos_from_index(pool: &SqlitePool, owner_id: i64) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM repository_index WHERE owner_id = ? AND is_private = 1",
    )
    .bind(owner_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

#[cfg(feature = "saas")]
pub async fn subscription_breakdown(pool: &SqlitePool) -> Result<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT plan, COUNT(*) FROM subscriptions WHERE status = 'active' GROUP BY plan",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(feature = "saas")]
pub async fn admin_override_plan(pool: &SqlitePool, user_id: i64, plan: &str) -> Result<()> {
    // Upsert a subscription with a sentinel stripe_customer_id for admin overrides
    sqlx::query(
        "INSERT INTO subscriptions (user_id, stripe_customer_id, plan, status, seats) \
         VALUES (?, 'admin_override_' || ?, ?, 'active', 1) \
         ON CONFLICT(user_id) DO UPDATE SET \
           plan = excluded.plan, \
           status = 'active', \
           updated_at = datetime('now')",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(plan)
    .execute(pool)
    .await?;
    Ok(())
}

// --- Organization queries ---

#[cfg(feature = "saas")]
pub async fn create_organization(
    pool: &SqlitePool,
    slug: &str,
    display_name: &str,
    created_by: i64,
) -> Result<Organization> {
    validate_username(slug)?; // same validation rules as usernames
    let org = sqlx::query_as::<_, Organization>(
        "INSERT INTO organizations (slug, display_name, created_by) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(slug)
    .bind(display_name)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(org)
}

#[cfg(feature = "saas")]
pub async fn get_organization_by_slug(pool: &SqlitePool, slug: &str) -> Result<Organization> {
    let org = sqlx::query_as::<_, Organization>("SELECT * FROM organizations WHERE slug = ?")
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(org)
}

#[cfg(feature = "saas")]
pub async fn list_user_organizations(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<(Organization, String)>> {
    let rows: Vec<(i64, String, String, i64, String, String, String)> = sqlx::query_as(
        "SELECT o.id, o.slug, o.display_name, o.created_by, o.created_at, o.updated_at, m.role \
         FROM organizations o \
         JOIN org_memberships m ON o.id = m.org_id \
         WHERE m.user_id = ? \
         ORDER BY o.slug",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, slug, display_name, created_by, created_at, updated_at, role)| {
                (
                    Organization {
                        id,
                        slug,
                        display_name,
                        created_by,
                        created_at,
                        updated_at,
                    },
                    role,
                )
            },
        )
        .collect())
}

#[cfg(feature = "saas")]
pub async fn add_org_member(
    pool: &SqlitePool,
    org_id: i64,
    user_id: i64,
    role: &str,
) -> Result<OrgMembership> {
    let membership = sqlx::query_as::<_, OrgMembership>(
        "INSERT INTO org_memberships (org_id, user_id, role) VALUES (?, ?, ?) RETURNING *",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(role)
    .fetch_one(pool)
    .await?;
    Ok(membership)
}

#[cfg(feature = "saas")]
pub async fn get_org_membership(
    pool: &SqlitePool,
    org_id: i64,
    user_id: i64,
) -> Result<Option<OrgMembership>> {
    let membership = sqlx::query_as::<_, OrgMembership>(
        "SELECT * FROM org_memberships WHERE org_id = ? AND user_id = ?",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(membership)
}

#[cfg(feature = "saas")]
pub async fn is_org_member(pool: &SqlitePool, org_id: i64, user_id: i64) -> Result<bool> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM org_memberships WHERE org_id = ? AND user_id = ?")
            .bind(org_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

#[cfg(feature = "saas")]
pub async fn remove_org_member(pool: &SqlitePool, org_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM org_memberships WHERE org_id = ? AND user_id = ?")
        .bind(org_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(feature = "saas")]
#[allow(clippy::type_complexity)]
pub async fn list_org_members(pool: &SqlitePool, org_id: i64) -> Result<Vec<(User, String)>> {
    let rows: Vec<(i64, String, String, String, String, bool, bool, String, String, String)> = sqlx::query_as(
        "SELECT u.id, u.username, u.email, u.password_hash, u.display_name, u.is_admin, u.is_disabled, \
                u.created_at, u.updated_at, m.role \
         FROM users u \
         JOIN org_memberships m ON u.id = m.user_id \
         WHERE m.org_id = ? \
         ORDER BY m.role, u.username",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                username,
                email,
                password_hash,
                display_name,
                is_admin,
                is_disabled,
                created_at,
                updated_at,
                role,
            )| {
                (
                    User {
                        id,
                        username,
                        email,
                        password_hash,
                        display_name,
                        is_admin,
                        is_disabled,
                        created_at,
                        updated_at,
                    },
                    role,
                )
            },
        )
        .collect())
}

// --- Repository index queries ---

/// Look up which org owns a repository by owner/repo name.
#[cfg(feature = "saas")]
pub async fn lookup_repo_org(
    pool: &SqlitePool,
    owner_username: &str,
    repo_name: &str,
) -> Result<String> {
    let (org_slug,): (String,) = sqlx::query_as(
        "SELECT org_slug FROM repository_index WHERE owner_username = ? AND repo_name = ?",
    )
    .bind(owner_username)
    .bind(repo_name)
    .fetch_optional(pool)
    .await?
    .ok_or(OxigitError::NotFound(
        "Repository not found in index".into(),
    ))?;
    Ok(org_slug)
}

/// Register a repository in the global index.
#[cfg(feature = "saas")]
pub async fn register_repo_in_index(
    pool: &SqlitePool,
    org_slug: &str,
    owner_id: i64,
    owner_username: &str,
    repo_name: &str,
    description: &str,
    is_private: bool,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO repository_index (org_slug, owner_id, owner_username, repo_name, description, is_private) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(owner_username, repo_name) DO UPDATE SET \
           org_slug = excluded.org_slug, \
           description = excluded.description, \
           is_private = excluded.is_private, \
           updated_at = datetime('now')",
    )
    .bind(org_slug)
    .bind(owner_id)
    .bind(owner_username)
    .bind(repo_name)
    .bind(description)
    .bind(is_private)
    .execute(pool)
    .await?;
    Ok(())
}

/// List repos owned by a user from the global index (for multi-tenant repo listing).
#[cfg(feature = "saas")]
pub async fn list_user_repos_from_index(
    pool: &SqlitePool,
    owner_id: i64,
) -> Result<Vec<crate::models::RepositoryIndexEntry>> {
    let rows = sqlx::query_as::<_, crate::models::RepositoryIndexEntry>(
        "SELECT id, org_slug, owner_id, owner_username, repo_name, description, is_private, created_at, updated_at \
         FROM repository_index WHERE owner_id = ? ORDER BY updated_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Search public repos from the global index (for multi-tenant explore page).
#[cfg(feature = "saas")]
pub async fn search_public_repos_from_index(
    pool: &SqlitePool,
    query: &str,
) -> Result<Vec<crate::models::RepositoryIndexEntry>> {
    let rows = if query.is_empty() {
        sqlx::query_as::<_, crate::models::RepositoryIndexEntry>(
            "SELECT id, org_slug, owner_id, owner_username, repo_name, description, is_private, created_at, updated_at \
             FROM repository_index WHERE is_private = 0 ORDER BY updated_at DESC LIMIT 50",
        )
        .fetch_all(pool)
        .await?
    } else {
        let pattern = format!("%{}%", query);
        sqlx::query_as::<_, crate::models::RepositoryIndexEntry>(
            "SELECT id, org_slug, owner_id, owner_username, repo_name, description, is_private, created_at, updated_at \
             FROM repository_index WHERE is_private = 0 AND (repo_name LIKE ? OR description LIKE ? OR owner_username LIKE ?) \
             ORDER BY updated_at DESC LIMIT 50",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

// --- Contact inquiries ---

pub async fn insert_contact_inquiry(
    pool: &SqlitePool,
    name: &str,
    email: &str,
    company: &str,
    message: &str,
) -> Result<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        "INSERT INTO contact_inquiries (name, email, company, message) VALUES (?, ?, ?, ?) RETURNING id"
    )
        .bind(name)
        .bind(email)
        .bind(company)
        .bind(message)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn list_contact_inquiries(pool: &SqlitePool) -> Result<Vec<ContactInquiry>> {
    let rows = sqlx::query_as::<_, ContactInquiry>(
        "SELECT id, name, email, company, message, created_at FROM contact_inquiries ORDER BY created_at DESC"
    )
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
