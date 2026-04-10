//! Shared migration logic: splits a legacy single `oxigit.db` into
//! a control-plane `control.db` and per-user tenant databases.

use std::path::Path;

use sqlx::sqlite::{Sqlite, SqlitePoolOptions, SqliteValueRef};
use sqlx::{Decode, Row, SqlitePool, TypeInfo, ValueRef};

use crate::db;

/// Migrate a legacy single-DB (`oxigit.db`) to multi-tenant layout.
///
/// This will:
/// 1. Open the existing `oxigit.db`
/// 2. Create `control.db` with control-plane schema and data
/// 3. For each user, create a personal org and tenant DB
/// 4. Copy tenant data (repos, issues, PRs, etc.) into each tenant DB
/// 5. Move git repo directories into tenant layout
/// 6. Rename `oxigit.db` to `oxigit.db.bak`
pub async fn migrate_legacy_to_multi_tenant(
    data_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let legacy_path = data_dir.join("oxigit.db");
    if !legacy_path.exists() {
        return Err(format!("No oxigit.db found at {}", legacy_path.display()).into());
    }

    let control_path = data_dir.join("control.db");
    if control_path.exists() {
        return Err("control.db already exists — migration may have already run.".into());
    }

    // Open legacy database
    let legacy_url = format!("sqlite:{}?mode=ro", legacy_path.display());
    let legacy = open_pool(&legacy_url).await?;

    tracing::info!("Opened legacy database at {}", legacy_path.display());

    // Create and migrate control database
    let control_url = format!("sqlite:{}?mode=rwc", control_path.display());
    let control = open_pool(&control_url).await?;
    db::run_control_migrations(&control).await?;

    tracing::info!("Created control.db with schema");

    // --- Copy control-plane data ---

    // Users
    let users: Vec<(i64, String, String, String, bool, bool, String, String)> = sqlx::query_as(
        "SELECT id, username, email, password_hash, is_admin, is_disabled, created_at, updated_at FROM users",
    )
    .fetch_all(&legacy)
    .await?;

    tracing::info!("Migrating {} users...", users.len());

    for (id, username, email, password_hash, is_admin, is_disabled, created_at, updated_at) in
        &users
    {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash, is_admin, is_disabled, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(username).bind(email).bind(password_hash)
        .bind(is_admin).bind(is_disabled).bind(created_at).bind(updated_at)
        .execute(&control)
        .await?;
    }

    // SSH keys
    copy_table_raw(
        &legacy,
        &control,
        "ssh_keys",
        "id, user_id, name, public_key, fingerprint, created_at",
    )
    .await?;

    // User settings
    copy_table_raw(
        &legacy,
        &control,
        "user_settings",
        "id, user_id, llm_provider, llm_api_key, llm_model, llm_base_url, updated_at",
    )
    .await?;

    // Subscriptions
    copy_table_raw(
        &legacy,
        &control,
        "subscriptions",
        "id, user_id, stripe_customer_id, stripe_subscription_id, plan, status, current_period_end, seats, created_at, updated_at",
    )
    .await?;

    // Founding members
    copy_table_raw(
        &legacy,
        &control,
        "founding_members",
        "id, user_id, slot_number, claimed_at",
    )
    .await?;

    // Organizations (may already exist if Phase 1 was deployed)
    let org_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM organizations")
        .fetch_one(&legacy)
        .await
        .unwrap_or((0,));

    if org_count.0 > 0 {
        copy_table_raw(
            &legacy,
            &control,
            "organizations",
            "id, slug, display_name, created_by, created_at, updated_at",
        )
        .await?;
        copy_table_raw(
            &legacy,
            &control,
            "org_memberships",
            "id, org_id, user_id, role, created_at",
        )
        .await?;
    }

    tracing::info!("Control-plane data copied.");

    // --- Create per-user tenant databases ---

    let tenants_dir = data_dir.join("tenants");
    std::fs::create_dir_all(&tenants_dir)?;

    for (user_id, username, ..) in &users {
        let user_id = *user_id;
        tracing::info!("  Provisioning tenant for user '{}'...", username);

        // Create org if it doesn't exist yet
        let existing_org: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM organizations WHERE slug = ?")
                .bind(username)
                .fetch_optional(&control)
                .await?;

        let org_id = if let Some((id,)) = existing_org {
            id
        } else {
            let result = sqlx::query(
                "INSERT INTO organizations (slug, display_name, created_by) VALUES (?, ?, ?) RETURNING id",
            )
            .bind(username)
            .bind(username)
            .bind(user_id)
            .execute(&control)
            .await?;
            result.last_insert_rowid()
        };

        // Ensure membership
        sqlx::query(
            "INSERT OR IGNORE INTO org_memberships (org_id, user_id, role) VALUES (?, ?, 'owner')",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&control)
        .await?;

        // Create tenant directory and database
        let tenant_dir = tenants_dir.join(username);
        let repos_dir = tenant_dir.join("repos");
        std::fs::create_dir_all(&repos_dir)?;

        let tenant_db_path = tenant_dir.join("tenant.db");
        let tenant_url = format!("sqlite:{}?mode=rwc", tenant_db_path.display());
        let tenant = open_pool(&tenant_url).await?;
        db::run_tenant_migrations(&tenant).await?;

        // Copy repositories for this user
        let repos: Vec<(i64, String, String, bool, Option<i64>, bool, String, String)> = sqlx::query_as(
            "SELECT id, name, description, is_private, forked_from, has_remix, created_at, updated_at \
             FROM repositories WHERE owner_id = ?",
        )
        .bind(user_id)
        .fetch_all(&legacy)
        .await?;

        for (
            repo_id,
            name,
            description,
            is_private,
            forked_from,
            has_remix,
            created_at,
            updated_at,
        ) in &repos
        {
            sqlx::query(
                "INSERT INTO repositories (id, owner_id, name, description, is_private, forked_from, has_remix, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(repo_id).bind(user_id).bind(name).bind(description)
            .bind(is_private).bind(forked_from).bind(has_remix)
            .bind(created_at).bind(updated_at)
            .execute(&tenant)
            .await?;

            // Register in repo index
            sqlx::query(
                "INSERT OR IGNORE INTO repository_index (org_slug, owner_id, owner_username, repo_name, description, is_private) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(username).bind(user_id).bind(username).bind(name)
            .bind(description).bind(is_private)
            .execute(&control)
            .await?;

            // Copy related tenant data for this repo
            copy_repo_data(&legacy, &tenant, *repo_id).await?;
        }

        // Move git repo directories
        let legacy_repos_dir = data_dir.join("repos").join(username);
        if legacy_repos_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&legacy_repos_dir) {
                for entry in entries.flatten() {
                    let src = entry.path();
                    let dest = repos_dir.join(username).join(entry.file_name());
                    std::fs::create_dir_all(dest.parent().unwrap())?;
                    if src.is_dir() {
                        if let Err(e) = std::fs::rename(&src, &dest) {
                            tracing::warn!("    rename failed ({}), copying instead...", e);
                            copy_dir_recursive(&src, &dest)?;
                            std::fs::remove_dir_all(&src)?;
                        }
                    }
                }
            }
        }

        tenant.close().await;
        tracing::info!("    {} repos migrated", repos.len());
    }

    // Close legacy DB and rename
    legacy.close().await;
    let backup_path = data_dir.join("oxigit.db.bak");
    std::fs::rename(&legacy_path, &backup_path)?;
    tracing::info!("Migration complete!");
    tracing::info!("  Legacy DB backed up to: {}", backup_path.display());
    tracing::info!("  Control DB: {}", control_path.display());
    tracing::info!("  Tenants: {}/tenants/*/tenant.db", data_dir.display());

    // Close the control pool so main.rs can open its own
    control.close().await;

    Ok(())
}

async fn open_pool(url: &str) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect(url)
        .await?;
    Ok(pool)
}

/// Copy all rows from a table between two databases using SQLite's text representation.
async fn copy_table_raw(
    src: &SqlitePool,
    dst: &SqlitePool,
    table: &str,
    columns: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let col_names: Vec<&str> = columns.split(',').map(|c| c.trim()).collect();
    let col_count = col_names.len();
    let select = format!("SELECT {} FROM {}", columns, table);
    let rows = sqlx::query(&select).fetch_all(src).await?;

    let placeholders: Vec<&str> = vec!["?"; col_count];
    let insert = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table,
        columns,
        placeholders.join(", ")
    );

    for row in &rows {
        let mut query = sqlx::query(&insert);
        for i in 0..col_count {
            let raw: SqliteValueRef<'_> = row.try_get_raw(i)?;
            if raw.is_null() {
                query = query.bind(None::<String>);
            } else {
                let type_info = raw.type_info().clone();
                let type_name = type_info.name();
                match type_name {
                    "INTEGER" | "BOOLEAN" => {
                        let v: i64 = Decode::<Sqlite>::decode(raw).map_err(|e| e.to_string())?;
                        query = query.bind(v);
                    }
                    "REAL" => {
                        let v: f64 = Decode::<Sqlite>::decode(raw).map_err(|e| e.to_string())?;
                        query = query.bind(v);
                    }
                    _ => {
                        let v: String = Decode::<Sqlite>::decode(raw).map_err(|e| e.to_string())?;
                        query = query.bind(v);
                    }
                }
            }
        }
        query.execute(dst).await?;
    }

    tracing::info!("  Copied {} rows from '{}'", rows.len(), table);
    Ok(())
}

/// Copy all repo-related tenant data for a single repository.
async fn copy_repo_data(
    legacy: &SqlitePool,
    tenant: &SqlitePool,
    repo_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Collaborators
    let rows: Vec<(i64, i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, user_id, permission, created_at FROM collaborators WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, user_id, permission, created_at) in rows {
        sqlx::query("INSERT INTO collaborators (id, repo_id, user_id, permission, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(id).bind(repo_id).bind(user_id).bind(permission).bind(created_at)
            .execute(tenant).await?;
    }

    // Pull requests
    let rows: Vec<(i64, i64, i64, String, String, i64, String, String, String, Option<i64>, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, number, title, description, author_id, source_branch, target_branch, status, merged_by, created_at, updated_at \
         FROM pull_requests WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (
        id,
        repo_id,
        number,
        title,
        description,
        author_id,
        source_branch,
        target_branch,
        status,
        merged_by,
        created_at,
        updated_at,
    ) in rows
    {
        sqlx::query(
            "INSERT INTO pull_requests (id, repo_id, number, title, description, author_id, source_branch, target_branch, status, merged_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(number).bind(title).bind(description).bind(author_id)
        .bind(source_branch).bind(target_branch).bind(status).bind(merged_by)
        .bind(created_at).bind(updated_at)
        .execute(tenant).await?;
    }

    // Issues
    let rows: Vec<(i64, i64, i64, String, String, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, number, title, description, author_id, status, created_at, updated_at \
         FROM issues WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, number, title, description, author_id, status, created_at, updated_at) in rows
    {
        sqlx::query(
            "INSERT INTO issues (id, repo_id, number, title, description, author_id, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(number).bind(title).bind(description).bind(author_id)
        .bind(status).bind(created_at).bind(updated_at)
        .execute(tenant).await?;
    }

    // Issue comments (via issue IDs)
    let issue_ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM issues WHERE repo_id = ?")
        .bind(repo_id)
        .fetch_all(legacy)
        .await?;
    for (issue_id,) in issue_ids {
        let comments: Vec<(i64, i64, i64, String, String)> = sqlx::query_as(
            "SELECT id, issue_id, author_id, body, created_at FROM issue_comments WHERE issue_id = ?",
        )
        .bind(issue_id)
        .fetch_all(legacy)
        .await?;
        for (id, issue_id, author_id, body, created_at) in comments {
            sqlx::query("INSERT INTO issue_comments (id, issue_id, author_id, body, created_at) VALUES (?, ?, ?, ?, ?)")
                .bind(id).bind(issue_id).bind(author_id).bind(body).bind(created_at)
                .execute(tenant).await?;
        }
    }

    // AI commit metadata
    let rows: Vec<(i64, i64, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, String)> = sqlx::query_as(
        "SELECT id, repo_id, commit_sha, ai_tool, ai_model, ai_prompt, ai_session_id, ai_files_touched, ai_prompt_index, created_at \
         FROM ai_commit_metadata WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (
        id,
        repo_id,
        commit_sha,
        ai_tool,
        ai_model,
        ai_prompt,
        ai_session_id,
        ai_files_touched,
        ai_prompt_index,
        created_at,
    ) in rows
    {
        sqlx::query(
            "INSERT INTO ai_commit_metadata (id, repo_id, commit_sha, ai_tool, ai_model, ai_prompt, ai_session_id, ai_files_touched, ai_prompt_index, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(commit_sha).bind(ai_tool).bind(ai_model).bind(ai_prompt)
        .bind(ai_session_id).bind(ai_files_touched).bind(ai_prompt_index).bind(created_at)
        .execute(tenant).await?;
    }

    // AI diff summaries
    let rows: Vec<(i64, i64, String, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, commit_sha, summary, risk_flags, generated_by, created_at \
         FROM ai_diff_summaries WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, commit_sha, summary, risk_flags, generated_by, created_at) in rows {
        sqlx::query(
            "INSERT INTO ai_diff_summaries (id, repo_id, commit_sha, summary, risk_flags, generated_by, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(commit_sha).bind(summary).bind(risk_flags)
        .bind(generated_by).bind(created_at)
        .execute(tenant).await?;
    }

    // Webhooks + deploy previews
    let rows: Vec<(i64, i64, String, Option<String>, String, bool, String)> = sqlx::query_as(
        "SELECT id, repo_id, url, secret, events, active, created_at FROM repo_webhooks WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, url, secret, events, active, created_at) in rows {
        sqlx::query("INSERT INTO repo_webhooks (id, repo_id, url, secret, events, active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(id).bind(repo_id).bind(url).bind(secret).bind(events).bind(active).bind(created_at)
            .execute(tenant).await?;
    }

    let rows: Vec<(
        i64,
        i64,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
    )> = sqlx::query_as(
        "SELECT id, repo_id, commit_sha, branch, preview_url, status, created_at, updated_at \
         FROM deploy_previews WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, commit_sha, branch, preview_url, status, created_at, updated_at) in rows {
        sqlx::query(
            "INSERT INTO deploy_previews (id, repo_id, commit_sha, branch, preview_url, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(commit_sha).bind(branch).bind(preview_url)
        .bind(status).bind(created_at).bind(updated_at)
        .execute(tenant).await?;
    }

    // Guardrails
    let rows: Vec<(i64, i64, String, String, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, category, action, created_at, updated_at FROM guardrail_rules WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, category, action, created_at, updated_at) in rows {
        sqlx::query("INSERT INTO guardrail_rules (id, repo_id, category, action, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id).bind(repo_id).bind(category).bind(action).bind(created_at).bind(updated_at)
            .execute(tenant).await?;
    }

    let rows: Vec<(i64, i64, Option<i64>, Option<i64>, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, min_vibe_score, max_files_per_push, created_at, updated_at FROM guardrail_config WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (id, repo_id, min_vibe_score, max_files_per_push, created_at, updated_at) in rows {
        sqlx::query("INSERT INTO guardrail_config (id, repo_id, min_vibe_score, max_files_per_push, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id).bind(repo_id).bind(min_vibe_score).bind(max_files_per_push).bind(created_at).bind(updated_at)
            .execute(tenant).await?;
    }

    let rows: Vec<(i64, i64, String, Option<String>, String, String, String, String, Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT id, repo_id, commit_sha, ref_name, rule_category, action_taken, severity, message, file_path, pushed_by, created_at \
         FROM guardrail_violations WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (
        id,
        repo_id,
        commit_sha,
        ref_name,
        rule_category,
        action_taken,
        severity,
        message,
        file_path,
        pushed_by,
        created_at,
    ) in rows
    {
        sqlx::query(
            "INSERT INTO guardrail_violations (id, repo_id, commit_sha, ref_name, rule_category, action_taken, severity, message, file_path, pushed_by, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(commit_sha).bind(ref_name).bind(rule_category)
        .bind(action_taken).bind(severity).bind(message).bind(file_path).bind(pushed_by).bind(created_at)
        .execute(tenant).await?;
    }

    // Recipes
    let recipes: Vec<(i64,)> = sqlx::query_as("SELECT id FROM recipes WHERE repo_id = ?")
        .bind(repo_id)
        .fetch_all(legacy)
        .await?;

    let rows: Vec<(i64, i64, String, i64, String, String, String, Option<String>, Option<String>, i64, i64, Option<i64>, i64, bool, String)> = sqlx::query_as(
        "SELECT id, repo_id, session_id, author_id, title, description, ai_tool, ai_model, tags, prompt_count, file_count, vibe_score, replay_count, is_public, created_at \
         FROM recipes WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (
        id,
        repo_id,
        session_id,
        author_id,
        title,
        description,
        ai_tool,
        ai_model,
        tags,
        prompt_count,
        file_count,
        vibe_score,
        replay_count,
        is_public,
        created_at,
    ) in rows
    {
        sqlx::query(
            "INSERT INTO recipes (id, repo_id, session_id, author_id, title, description, ai_tool, ai_model, tags, prompt_count, file_count, vibe_score, replay_count, is_public, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(session_id).bind(author_id).bind(title).bind(description)
        .bind(ai_tool).bind(ai_model).bind(tags).bind(prompt_count).bind(file_count)
        .bind(vibe_score).bind(replay_count).bind(is_public).bind(created_at)
        .execute(tenant).await?;
    }

    // Recipe steps and replays (via recipe IDs)
    for (recipe_id,) in recipes {
        let steps: Vec<(i64, i64, i64, Option<String>, Option<i64>, String, Option<String>, Option<String>, String)> = sqlx::query_as(
            "SELECT id, recipe_id, step_order, prompt_text, prompt_index, commit_message, files_json, diff_text, created_at \
             FROM recipe_steps WHERE recipe_id = ?",
        )
        .bind(recipe_id)
        .fetch_all(legacy)
        .await?;
        for (
            id,
            recipe_id,
            step_order,
            prompt_text,
            prompt_index,
            commit_message,
            files_json,
            diff_text,
            created_at,
        ) in steps
        {
            sqlx::query(
                "INSERT INTO recipe_steps (id, recipe_id, step_order, prompt_text, prompt_index, commit_message, files_json, diff_text, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id).bind(recipe_id).bind(step_order).bind(prompt_text).bind(prompt_index)
            .bind(commit_message).bind(files_json).bind(diff_text).bind(created_at)
            .execute(tenant).await?;
        }

        let replays: Vec<(i64, i64, i64, i64, String, String, String, i64, Option<String>, String)> = sqlx::query_as(
            "SELECT id, recipe_id, user_id, target_repo_id, target_branch, mode, status, steps_applied, error_message, created_at \
             FROM recipe_replays WHERE recipe_id = ?",
        )
        .bind(recipe_id)
        .fetch_all(legacy)
        .await?;
        for (
            id,
            recipe_id,
            user_id,
            target_repo_id,
            target_branch,
            mode,
            status,
            steps_applied,
            error_message,
            created_at,
        ) in replays
        {
            sqlx::query(
                "INSERT INTO recipe_replays (id, recipe_id, user_id, target_repo_id, target_branch, mode, status, steps_applied, error_message, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id).bind(recipe_id).bind(user_id).bind(target_repo_id).bind(target_branch)
            .bind(mode).bind(status).bind(steps_applied).bind(error_message).bind(created_at)
            .execute(tenant).await?;
        }
    }

    // Merge conflicts
    let conflicts: Vec<(i64, i64, i64, String, String, String, String, Option<String>, Option<String>, String, String, String)> = sqlx::query_as(
        "SELECT id, repo_id, user_id, operation_type, target_ref, source_ref, merge_base, auto_tree, context_json, status, created_at, updated_at \
         FROM merge_conflicts WHERE repo_id = ?",
    )
    .bind(repo_id)
    .fetch_all(legacy)
    .await?;
    for (
        id,
        repo_id,
        user_id,
        operation_type,
        target_ref,
        source_ref,
        merge_base,
        auto_tree,
        context_json,
        status,
        created_at,
        updated_at,
    ) in &conflicts
    {
        sqlx::query(
            "INSERT INTO merge_conflicts (id, repo_id, user_id, operation_type, target_ref, source_ref, merge_base, auto_tree, context_json, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id).bind(repo_id).bind(user_id).bind(operation_type).bind(target_ref).bind(source_ref)
        .bind(merge_base).bind(auto_tree).bind(context_json).bind(status).bind(created_at).bind(updated_at)
        .execute(tenant).await?;
    }
    for (conflict_id, ..) in &conflicts {
        let files: Vec<(i64, i64, String, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, merge_conflict_id, file_path, conflict_type, resolution, resolved_content, resolved_at \
             FROM merge_conflict_files WHERE merge_conflict_id = ?",
        )
        .bind(conflict_id)
        .fetch_all(legacy)
        .await?;
        for (
            id,
            merge_conflict_id,
            file_path,
            conflict_type,
            resolution,
            resolved_content,
            resolved_at,
        ) in files
        {
            sqlx::query(
                "INSERT INTO merge_conflict_files (id, merge_conflict_id, file_path, conflict_type, resolution, resolved_content, resolved_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id).bind(merge_conflict_id).bind(file_path).bind(conflict_type)
            .bind(resolution).bind(resolved_content).bind(resolved_at)
            .execute(tenant).await?;
        }
    }

    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
