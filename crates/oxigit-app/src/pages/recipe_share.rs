use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;

#[server]
async fn share_session_as_recipe(
    owner: String,
    repo: String,
    session_id: String,
    title: String,
    description: String,
    tags: String,
) -> Result<i64, ServerFnError> {
    use crate::server_fns::{get_repo_path, get_repo_pools, require_auth, sfn_err};
    use oxigit_core::{db, git, vibe};

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let can_push = db::can_push_repo(&pool, &repo_db, user.id)
        .await
        .map_err(sfn_err)?;
    if !can_push {
        return Err(ServerFnError::new("Access denied"));
    }

    // Check if already shared
    if let Ok(Some(_)) = db::get_recipe_for_session(&pool, repo_db.id, &session_id).await {
        return Err(ServerFnError::new(
            "This session is already shared as a recipe",
        ));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;

    let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &session_id)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Session not found"));
    }

    let ai_tool = metas[0].ai_tool.clone();
    let ai_model = metas[0].ai_model.clone();

    // Group commits by prompt_index (or by prompt text for ungrouped)
    let mut prompt_groups: Vec<(Option<String>, Vec<&oxigit_core::models::AiCommitMetadata>)> =
        Vec::new();
    for meta in &metas {
        let key = meta.ai_prompt.clone();
        if let Some(group) = prompt_groups.iter_mut().find(|(k, _)| *k == key) {
            group.1.push(meta);
        } else {
            prompt_groups.push((key, vec![meta]));
        }
    }

    // Compute vibe score
    let shas: Vec<String> = metas.iter().rev().map(|m| m.commit_sha.clone()).collect();
    let (lines_added, lines_deleted) = git::session_diff_stats(&repo_path, &shas).unwrap_or((0, 0));
    let metrics = vibe::SessionMetrics {
        commit_count: metas.len(),
        prompt_count: prompt_groups.len(),
        risk_flag_count: 0,
        files_touched: 0,
        lines_added,
        lines_deleted,
        was_reverted: false,
    };
    let vs = vibe::compute_vibe_score(&metrics);

    // Collect all unique files
    let mut all_files: Vec<String> = Vec::new();
    for meta in &metas {
        if let Some(ref fj) = meta.ai_files_touched
            && let Ok(files) = serde_json::from_str::<Vec<String>>(fj)
        {
            for f in files {
                if !all_files.contains(&f) {
                    all_files.push(f);
                }
            }
        }
    }

    let tags_opt = if tags.trim().is_empty() {
        None
    } else {
        let tag_list: Vec<String> = tags
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        Some(serde_json::to_string(&tag_list).unwrap_or_default())
    };

    let recipe = db::create_recipe(
        &pool,
        repo_db.id,
        &session_id,
        user.id,
        &title,
        &description,
        &ai_tool,
        ai_model.as_deref(),
        tags_opt.as_deref(),
        prompt_groups.len() as i64,
        all_files.len() as i64,
        Some(vs.score as i64),
    )
    .await
    .map_err(sfn_err)?;

    // Create steps
    for (step_idx, (prompt_text, group_metas)) in prompt_groups.iter().enumerate() {
        let last_sha = &group_metas[0].commit_sha; // metas are DESC, first is latest
        let commit_message = git::get_latest_commit(&repo_path, last_sha)
            .ok()
            .flatten()
            .map(|c| c.message)
            .unwrap_or_else(|| "unknown".into());

        // Read file contents at this commit
        let files: Vec<serde_json::Value> = all_files.iter().filter_map(|f| {
            git::read_blob(&repo_path, last_sha, f).ok().map(|content| {
                serde_json::json!({ "path": f, "content": String::from_utf8_lossy(&content).to_string() })
            })
        }).collect();
        let files_json = serde_json::to_string(&files).ok();

        // Get diff for this step
        let step_shas: Vec<String> = group_metas
            .iter()
            .rev()
            .map(|m| m.commit_sha.clone())
            .collect();
        let diff = git::session_aggregate_diff(&repo_path, &step_shas).unwrap_or_default();

        let _ = db::create_recipe_step(
            &pool,
            recipe.id,
            step_idx as i64,
            prompt_text.as_deref(),
            group_metas[0].ai_prompt_index,
            &commit_message,
            files_json.as_deref(),
            Some(&diff),
        )
        .await;
    }

    leptos_axum::redirect(&format!("/recipes/{}", recipe.id));
    Ok(recipe.id)
}

#[component]
pub fn ShareRecipePage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let session_id = move || params.read().get("session_id").unwrap_or_default();

    let share_action = ServerAction::<ShareSessionAsRecipe>::new();
    let error = move || share_action.value().get().and_then(|r| r.err());

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <a href={move || format!("/{}/{}/ai/{}", owner(), repo(), session_id())}>"AI Session"</a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Share"</span>
            </h1>
        </div>

        {move || error().map(|e| view! {
            <ErrorDisplay error=e.to_string() />
        })}

        <div class="card">
            <div class="card-header">"Share Session as Recipe"</div>
            <div style="padding: var(--space-4);">
                <p class="text-secondary mb-3" style="font-size: 0.8125rem;">
                    "Share this AI session's prompts and file changes as a reusable recipe. Others can discover and replay it."
                </p>
                <ActionForm action=share_action>
                    <input type="hidden" name="owner" value={move || owner()} />
                    <input type="hidden" name="repo" value={move || repo()} />
                    <input type="hidden" name="session_id" value={move || session_id()} />
                    <div class="form-group">
                        <label>"Title"</label>
                        <input type="text" name="title" required placeholder="What does this recipe do?" class="form-input" />
                    </div>
                    <div class="form-group">
                        <label>"Description"</label>
                        <textarea name="description" rows="3" placeholder="Describe the recipe..." class="form-input"></textarea>
                    </div>
                    <div class="form-group">
                        <label>"Tags (comma-separated)"</label>
                        <input type="text" name="tags" placeholder="rust, refactor, testing" class="form-input" />
                    </div>
                    <button type="submit" class="btn btn-primary">"Share Recipe"</button>
                </ActionForm>
            </div>
        </div>
    }
}
