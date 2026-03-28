use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

use crate::auth::{hash_password, validate_repo_name, validate_username, verify_password};
use crate::error::{OxigitError, Result};
use crate::models::{Repository, User};

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
