use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

use crate::auth::{hash_password, validate_repo_name, validate_username, verify_password};
use crate::error::{OxigitError, Result};
use crate::models::{AiCommitMetadata, AiDiffSummary, Collaborator, Issue, IssueComment, PullRequest, Repository, SshKey, User, UserSettings};

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("../../migrations").run(pool).await.map_err(|e| {
        OxigitError::Database(sqlx::Error::Protocol(format!("Migration failed: {e}")))
    })?;
    Ok(())
}

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

pub async fn authenticate_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<User> {
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

/// Check if a user can access a repository. Public repos are accessible to all.
/// Private repos require the viewer to be the owner.
pub fn can_access_repo(repo: &Repository, viewer_id: Option<i64>) -> bool {
    if !repo.is_private {
        return true;
    }
    viewer_id == Some(repo.owner_id)
}

/// Search public repositories. If query is empty, returns all public repos.
pub async fn search_public_repositories(pool: &SqlitePool, query: &str) -> Result<Vec<(User, Repository)>> {
    let rows = if query.is_empty() {
        sqlx::query_as::<_, (i64, String, String, String, i64, String, String, bool, String, String)>(
            "SELECT u.id, u.username, u.email, u.password_hash, r.id, r.name, r.description, r.is_private, r.created_at, r.updated_at \
             FROM repositories r JOIN users u ON r.owner_id = u.id \
             WHERE r.is_private = 0 \
             ORDER BY r.updated_at DESC LIMIT 50",
        )
        .fetch_all(pool)
        .await?
    } else {
        let pattern = format!("%{}%", query);
        sqlx::query_as::<_, (i64, String, String, String, i64, String, String, bool, String, String)>(
            "SELECT u.id, u.username, u.email, u.password_hash, r.id, r.name, r.description, r.is_private, r.created_at, r.updated_at \
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
        .map(|(uid, username, email, pw, rid, name, desc, private, created, updated)| {
            (
                User { id: uid, username, email, password_hash: pw, created_at: created.clone(), updated_at: updated.clone() },
                Repository { id: rid, owner_id: uid, name, description: desc, is_private: private, forked_from: None, created_at: created, updated_at: updated },
            )
        })
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
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE") => {
            OxigitError::InvalidInput("This SSH key is already registered".into())
        }
        _ => e.into(),
    })?;
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
    let key = sqlx::query_as::<_, SshKey>(
        "SELECT * FROM ssh_keys WHERE fingerprint = ?",
    )
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
        return Err(OxigitError::InvalidInput("Cannot fork your own repository".into()));
    }

    // Can't fork private repos you don't have access to
    if source_repo.is_private && !can_access_repo(&source_repo, Some(fork_user_id)) {
        return Err(OxigitError::NotFound("Repository not found".into()));
    }

    let fork_user = get_user_by_id(pool, fork_user_id).await?;

    // Check if fork already exists
    if get_repository(pool, &fork_user.username, source_name).await.is_ok() {
        return Err(OxigitError::InvalidInput("You already have a repository with this name".into()));
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

/// Get the source repo info for a fork.
pub async fn get_fork_source(pool: &SqlitePool, repo: &Repository) -> Result<Option<(User, Repository)>> {
    match repo.forked_from {
        Some(source_id) => {
            let source = sqlx::query_as::<_, Repository>("SELECT * FROM repositories WHERE id = ?")
                .bind(source_id)
                .fetch_optional(pool)
                .await?;
            match source {
                Some(source_repo) => {
                    let owner = get_user_by_id(pool, source_repo.owner_id).await?;
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
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.message().contains("UNIQUE") => {
            OxigitError::InvalidInput("User is already a collaborator".into())
        }
        _ => e.into(),
    })?;
    Ok(collab)
}

pub async fn list_collaborators(pool: &SqlitePool, repo_id: i64) -> Result<Vec<(User, Collaborator)>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, String, i64, i64, i64, String, String)>(
        "SELECT u.id, u.username, u.email, u.password_hash, u.created_at, u.updated_at, \
         c.id, c.repo_id, c.user_id, c.permission, c.created_at \
         FROM collaborators c JOIN users u ON c.user_id = u.id \
         WHERE c.repo_id = ? ORDER BY c.created_at",
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(uid, username, email, pw, ucreated, uupdated, cid, repo_id, user_id, perm, ccreated)| {
            (
                User { id: uid, username, email, password_hash: pw, created_at: ucreated, updated_at: uupdated },
                Collaborator { id: cid, repo_id, user_id, permission: perm, created_at: ccreated },
            )
        })
        .collect())
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
        return Err(OxigitError::InvalidInput("Source and target branches must differ".into()));
    }

    // Get next PR number for this repo
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM pull_requests WHERE repo_id = ?",
    )
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

pub async fn list_pull_requests(
    pool: &SqlitePool,
    repo_id: i64,
    status: Option<&str>,
) -> Result<Vec<(PullRequest, User)>> {
    let prs = if let Some(status) = status {
        sqlx::query_as::<_, PullRequest>(
            "SELECT * FROM pull_requests WHERE repo_id = ? AND status = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .bind(status)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, PullRequest>(
            "SELECT * FROM pull_requests WHERE repo_id = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .fetch_all(pool)
        .await?
    };

    let mut result = Vec::new();
    for pr in prs {
        let author = get_user_by_id(pool, pr.author_id).await?;
        result.push((pr, author));
    }
    Ok(result)
}

pub async fn get_pull_request(
    pool: &SqlitePool,
    repo_id: i64,
    number: i64,
) -> Result<PullRequest> {
    sqlx::query_as::<_, PullRequest>(
        "SELECT * FROM pull_requests WHERE repo_id = ? AND number = ?",
    )
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

    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE repo_id = ?",
    )
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

pub async fn list_issues(
    pool: &SqlitePool,
    repo_id: i64,
    status: Option<&str>,
) -> Result<Vec<(Issue, User)>> {
    let issues = if let Some(status) = status {
        sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE repo_id = ? AND status = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .bind(status)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Issue>(
            "SELECT * FROM issues WHERE repo_id = ? ORDER BY updated_at DESC",
        )
        .bind(repo_id)
        .fetch_all(pool)
        .await?
    };

    let mut result = Vec::new();
    for issue in issues {
        let author = get_user_by_id(pool, issue.author_id).await?;
        result.push((issue, author));
    }
    Ok(result)
}

pub async fn get_issue(pool: &SqlitePool, repo_id: i64, number: i64) -> Result<Issue> {
    sqlx::query_as::<_, Issue>(
        "SELECT * FROM issues WHERE repo_id = ? AND number = ?",
    )
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
        .bind(issue_id).execute(pool).await?;

    Ok(comment)
}

pub async fn list_issue_comments(pool: &SqlitePool, issue_id: i64) -> Result<Vec<(IssueComment, User)>> {
    let comments = sqlx::query_as::<_, IssueComment>(
        "SELECT * FROM issue_comments WHERE issue_id = ? ORDER BY created_at ASC",
    )
    .bind(issue_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for comment in comments {
        let author = get_user_by_id(pool, comment.author_id).await?;
        result.push((comment, author));
    }
    Ok(result)
}

// --- AI Commit Metadata queries ---

pub async fn insert_ai_metadata(
    pool: &SqlitePool,
    repo_id: i64,
    commit_sha: &str,
    ai_tool: &str,
    ai_model: Option<&str>,
    ai_prompt: Option<&str>,
    ai_session_id: Option<&str>,
    ai_files_touched: Option<&str>,
) -> Result<AiCommitMetadata> {
    let meta = sqlx::query_as::<_, AiCommitMetadata>(
        "INSERT OR REPLACE INTO ai_commit_metadata (repo_id, commit_sha, ai_tool, ai_model, ai_prompt, ai_session_id, ai_files_touched) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(repo_id)
    .bind(commit_sha)
    .bind(ai_tool)
    .bind(ai_model)
    .bind(ai_prompt)
    .bind(ai_session_id)
    .bind(ai_files_touched)
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
pub async fn list_ai_sessions(
    pool: &SqlitePool,
    repo_id: i64,
) -> Result<Vec<AiCommitMetadata>> {
    let metas = sqlx::query_as::<_, AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? ORDER BY ai_session_id, created_at ASC",
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;
    Ok(metas)
}

/// Get all AI metadata for a specific session.
pub async fn get_ai_metadata_by_session(
    pool: &SqlitePool,
    repo_id: i64,
    session_id: &str,
) -> Result<Vec<AiCommitMetadata>> {
    let metas = sqlx::query_as::<_, AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? AND ai_session_id = ? ORDER BY created_at ASC",
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

// --- User Settings queries ---

pub async fn get_user_settings(pool: &SqlitePool, user_id: i64) -> Result<Option<UserSettings>> {
    let settings = sqlx::query_as::<_, UserSettings>(
        "SELECT * FROM user_settings WHERE user_id = ?",
    )
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
) -> Result<UserSettings> {
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
    .bind(llm_api_key)
    .bind(llm_model)
    .bind(llm_base_url)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
