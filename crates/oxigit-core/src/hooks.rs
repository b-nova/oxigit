use std::collections::HashMap;
use std::path::Path;

use sqlx::sqlite::SqlitePool;
use tracing;

use crate::db;
use crate::git;

/// Process post-receive hook: parse AI trailers from new commits and store metadata.
/// Compares before/after ref snapshots to find new commits, then parses their messages.
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
            let message = match git::get_full_commit_message(repo_path, sha) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("Failed to get commit message for {}: {}", sha, e);
                    continue;
                }
            };

            if let Some(trailers) = git::parse_ai_trailers(&message) {
                if let Err(e) = db::insert_ai_metadata(
                    pool,
                    repo_id,
                    sha,
                    &trailers.ai_tool,
                    trailers.ai_model.as_deref(),
                    trailers.ai_prompt.as_deref(),
                    trailers.ai_session_id.as_deref(),
                    trailers.ai_files_touched.as_deref(),
                )
                .await
                {
                    tracing::warn!("Failed to insert AI metadata for commit {}: {}", sha, e);
                }
            }
        }
    }
}
