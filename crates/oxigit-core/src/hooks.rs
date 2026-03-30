use std::collections::HashMap;
use std::path::Path;

use sqlx::sqlite::SqlitePool;
use tracing;

use crate::db;
use crate::git;

/// Process post-receive hook: detect AI metadata from `.oxigit/context.json` in new commits.
/// Compares before/after ref snapshots to find new commits, reads context files, and auto-detects changed files.
pub async fn process_post_receive(
    pool: &SqlitePool,
    repo_path: &Path,
    repo_id: i64,
    before_refs: &HashMap<String, String>,
    after_refs: &HashMap<String, String>,
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

        for sha in &shas {
            let context = match git::read_oxigit_context(repo_path, sha) {
                Ok(Some(ctx)) => ctx,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!("Failed to read .oxigit/context.json for {}: {}", sha, e);
                    continue;
                }
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
            )
            .await
            {
                tracing::warn!("Failed to insert AI metadata for commit {}: {}", sha, e);
            }
        }
    }
}
