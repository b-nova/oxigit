use std::collections::HashMap;
use std::path::Path;

use sqlx::sqlite::SqlitePool;
use tracing;

use crate::db;
use crate::git;
use crate::guardrail;
use crate::webhook;

/// Process post-receive hook: detect AI metadata from `.oxigit/context.json` in new commits.
/// Compares before/after ref snapshots to find new commits, reads context files, and auto-detects changed files.
pub async fn process_post_receive(
    pool: &SqlitePool,
    repo_path: &Path,
    repo_id: i64,
    owner_id: i64,
    before_refs: &HashMap<String, String>,
    after_refs: &HashMap<String, String>,
    owner: &str,
    repo_name: &str,
    callback_base_url: &str,
) {
    let zero_sha = "0000000000000000000000000000000000000000";

    for (refname, new_sha) in after_refs {
        let old_sha = before_refs.get(refname).map(|s| s.as_str()).unwrap_or(zero_sha);

        // Skip if ref didn't change
        if old_sha == new_sha {
            continue;
        }

        // Only process branch refs
        if !refname.starts_with("refs/heads/") {
            continue;
        }

        // For new branches, we need to exclude commits already reachable from
        // refs that existed before the push (not from current branches, which
        // now include the new branch).
        let shas = if old_sha == zero_sha {
            // New branch: list commits reachable from new_sha but not from any pre-existing ref
            let exclude_refs: Vec<&str> = before_refs.values().map(|s| s.as_str()).collect();
            match git::list_new_commit_shas_excluding(repo_path, new_sha, &exclude_refs) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to list new commits for {}: {}", refname, e);
                    continue;
                }
            }
        } else {
            match git::list_new_commit_shas(repo_path, old_sha, new_sha) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to list new commits for {}: {}", refname, e);
                    continue;
                }
            }
        };

        // Check if REMIX.md exists at the tip of the pushed branch
        if let Ok(content) = git::read_blob(repo_path, new_sha, "REMIX.md") {
            let has_remix = !content.is_empty();
            let _ = db::update_has_remix(pool, repo_id, has_remix).await;
        }

        for sha in &shas {
            // Try trailers first (new approach), fall back to context.json (legacy)
            let context = match git::read_oxigit_trailers(repo_path, sha) {
                Ok(Some(ctx)) => ctx,
                _ => match git::read_oxigit_context(repo_path, sha) {
                    Ok(Some(ctx)) => ctx,
                    Ok(None) => continue,
                    Err(e) => {
                        tracing::warn!("Failed to read AI metadata for {}: {}", sha, e);
                        continue;
                    }
                },
            };

            let files = git::list_changed_files(repo_path, sha).unwrap_or_default();
            let files_json = if files.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&files).unwrap_or_default())
            };

            if let Err(e) = db::insert_ai_metadata(
                pool,
                repo_id,
                sha,
                &context.tool,
                context.model.as_deref(),
                context.prompt.as_deref(),
                context.session_id.as_deref(),
                files_json.as_deref(),
                context.prompt_index,
            )
            .await
            {
                tracing::warn!("Failed to insert AI metadata for commit {}: {}", sha, e);
            }
        }

        // Run guardrail warn-level scanning on new commits
        let guardrail_rules = db::get_guardrail_rules(pool, repo_id).await.unwrap_or_default();
        let guardrail_config = db::get_guardrail_config(pool, repo_id).await.ok().flatten();
        let warn_rules: Vec<_> = guardrail_rules.iter().filter(|r| r.action == "warn").cloned().collect();

        if !warn_rules.is_empty() || guardrail_config.as_ref().map(|c| c.max_files_per_push.is_some()).unwrap_or(false) {
            for sha in &shas {
                let diff = git::show_commit_diff(repo_path, sha)
                    .ok()
                    .map(|(_, d)| d)
                    .unwrap_or_default();
                let file_count = git::list_changed_files(repo_path, sha).unwrap_or_default().len();

                let violations = guardrail::evaluate_diff(&warn_rules, &guardrail_config, &diff, file_count);
                let warn_violations: Vec<_> = violations.iter().filter(|v| v.action == "warn").collect();

                for v in &warn_violations {
                    let _ = db::insert_guardrail_violation(
                        pool, repo_id, sha, Some(refname),
                        &v.category, "warned", &v.severity, &v.message,
                        v.file_path.as_deref(), None,
                    ).await;
                }

                if !warn_violations.is_empty() {
                    tracing::info!(
                        "Guardrail warnings for commit {} in {}/{}: {} violation(s)",
                        &sha[..7.min(sha.len())], owner, repo_name, warn_violations.len()
                    );
                }
            }
        }

        // Fire webhooks for this push
        if let Ok(webhooks) = db::get_active_webhooks(pool, repo_id).await {
            if !webhooks.is_empty() {
                let branch = refname.strip_prefix("refs/heads/").unwrap_or(refname);
                let commit_msg = git::get_latest_commit(repo_path, new_sha)
                    .ok()
                    .flatten()
                    .map(|c| c.message)
                    .unwrap_or_default();

                // Create pending deploy preview (Pro+ only)
                let owner_plan = db::get_user_plan(pool, owner_id)
                    .await
                    .unwrap_or_else(|_| "free".into());
                let ent = crate::entitlements::for_plan(&owner_plan);
                if ent.deploy_previews {
                    let _ = db::create_deploy_preview(pool, repo_id, new_sha, branch).await;
                }

                webhook::fire_push_webhooks(
                    &webhooks, owner, repo_name, branch, new_sha, &commit_msg, callback_base_url,
                )
                .await;
            }
        }
    }
}
