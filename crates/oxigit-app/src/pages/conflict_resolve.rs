use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

use super::{ConflictDetailResponse, ConflictFileContentResponse};

#[server]
async fn fetch_conflict_detail(
    owner: String,
    repo: String,
    conflict_id: i64,
) -> Result<ConflictDetailResponse, ServerFnError> {
    use super::ConflictFileInfo;
    use crate::server_fns::{get_repo_pools, require_auth, sfn_err};
    use oxigit_core::db;

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let conflict = db::get_merge_conflict(&pool, conflict_id)
        .await
        .map_err(sfn_err)?
        .ok_or_else(|| ServerFnError::new("Conflict not found"))?;

    if conflict.repo_id != repo_db.id || conflict.user_id != user.id {
        return Err(ServerFnError::new("Access denied"));
    }

    let files = db::get_conflict_files(&pool, conflict_id)
        .await
        .map_err(sfn_err)?;

    let file_infos: Vec<ConflictFileInfo> = files
        .iter()
        .map(|f| ConflictFileInfo {
            id: f.id,
            file_path: f.file_path.clone(),
            conflict_type: f.conflict_type.clone(),
            resolution: f.resolution.clone(),
            is_resolved: f.resolution.is_some(),
        })
        .collect();

    let all_resolved = file_infos.iter().all(|f| f.is_resolved);

    Ok(ConflictDetailResponse {
        id: conflict.id,
        operation_type: conflict.operation_type,
        target_ref: conflict.target_ref,
        source_ref: conflict.source_ref,
        status: conflict.status,
        files: file_infos,
        all_resolved,
        owner,
        repo,
    })
}

#[server]
async fn fetch_conflict_file_content(
    owner: String,
    repo: String,
    conflict_id: i64,
    file_path: String,
) -> Result<ConflictFileContentResponse, ServerFnError> {
    use crate::server_fns::{get_repo_path, get_repo_pools, require_auth, sfn_err};
    use oxigit_core::{db, git};

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let conflict = db::get_merge_conflict(&pool, conflict_id)
        .await
        .map_err(sfn_err)?
        .ok_or_else(|| ServerFnError::new("Conflict not found"))?;

    if conflict.repo_id != repo_db.id || conflict.user_id != user.id {
        return Err(ServerFnError::new("Access denied"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;

    let ours = git::read_blob(&repo_path, &conflict.target_ref, &file_path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).to_string());
    let theirs = git::read_blob(&repo_path, &conflict.source_ref, &file_path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).to_string());

    // Check if this file already has a resolution saved
    let files = db::get_conflict_files(&pool, conflict_id)
        .await
        .map_err(sfn_err)?;
    let file_record = files.iter().find(|f| f.file_path == file_path);
    let resolved_content = file_record.and_then(|f| f.resolved_content.clone());

    let (ours_label, theirs_label) = if conflict.operation_type == "revert" {
        let short_target = &conflict.target_ref[..7.min(conflict.target_ref.len())];
        let short_source = &conflict.source_ref[..7.min(conflict.source_ref.len())];
        (
            format!("Current ({})", short_target),
            format!("Reverted ({})", short_source),
        )
    } else {
        (conflict.target_ref, conflict.source_ref)
    };

    Ok(ConflictFileContentResponse {
        file_path,
        ours_content: ours,
        theirs_content: theirs,
        resolved_content,
        ours_label,
        theirs_label,
    })
}

#[server]
async fn resolve_file(
    owner: String,
    repo: String,
    conflict_id: i64,
    file_id: i64,
    resolution: String,
    manual_content: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_repo_path, get_repo_pools, require_auth, sfn_err};
    use oxigit_core::{db, git};

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let conflict = db::get_merge_conflict(&pool, conflict_id)
        .await
        .map_err(sfn_err)?
        .ok_or_else(|| ServerFnError::new("Conflict not found"))?;

    if conflict.repo_id != repo_db.id || conflict.user_id != user.id {
        return Err(ServerFnError::new("Access denied"));
    }

    // Get file info for the path
    let files = db::get_conflict_files(&pool, conflict_id)
        .await
        .map_err(sfn_err)?;
    let file_record = files
        .iter()
        .find(|f| f.id == file_id)
        .ok_or_else(|| ServerFnError::new("File not found"))?;

    let repo_path = get_repo_path(&owner, &repo).await?;

    let content: Option<String> = match resolution.as_str() {
        "ours" => git::read_blob(&repo_path, &conflict.target_ref, &file_record.file_path)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).to_string()),
        "theirs" => git::read_blob(&repo_path, &conflict.source_ref, &file_record.file_path)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).to_string()),
        "manual" => manual_content,
        _ => return Err(ServerFnError::new("Invalid resolution")),
    };

    db::resolve_conflict_file(&pool, file_id, &resolution, content.as_deref())
        .await
        .map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/conflicts/{}", owner, repo, conflict_id));
    Ok(())
}

#[server]
async fn complete_resolution(
    owner: String,
    repo: String,
    conflict_id: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_repo_path, get_repo_pools, require_auth, sfn_err};
    use oxigit_core::{db, git};

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let conflict = db::get_merge_conflict(&pool, conflict_id)
        .await
        .map_err(sfn_err)?
        .ok_or_else(|| ServerFnError::new("Conflict not found"))?;

    if conflict.repo_id != repo_db.id || conflict.user_id != user.id {
        return Err(ServerFnError::new("Access denied"));
    }

    let files = db::get_conflict_files(&pool, conflict_id)
        .await
        .map_err(sfn_err)?;

    // Verify all files resolved
    if files.iter().any(|f| f.resolution.is_none()) {
        return Err(ServerFnError::new("Not all files are resolved"));
    }

    let auto_tree = conflict
        .auto_tree
        .ok_or_else(|| ServerFnError::new("No auto-merged tree available"))?;

    // Build resolutions list
    let resolutions: Vec<(String, String)> = files
        .iter()
        .filter_map(|f| {
            f.resolved_content
                .as_ref()
                .map(|c| (f.file_path.clone(), c.clone()))
        })
        .collect();

    let repo_path = get_repo_path(&owner, &repo).await?;

    if conflict.operation_type == "revert" {
        // Revert: single-parent commit, then continue reverting remaining SHAs
        let ctx: serde_json::Value = conflict
            .context_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));

        let branch = ctx.get("branch").and_then(|v| v.as_str()).unwrap_or("main");
        let revert_message = ctx
            .get("revert_message")
            .and_then(|v| v.as_str())
            .unwrap_or("Revert");
        let remaining_shas: Vec<String> = ctx
            .get("remaining_shas")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // Get the current branch tip to use as parent
        let branch_ref = format!("refs/heads/{}", branch);
        let branch_tip = git::rev_parse(&repo_path, &branch_ref).map_err(sfn_err)?;

        // Create the resolution commit with a single parent (branch tip)
        let resolve_msg = format!(
            "Resolve revert conflict: {}",
            &conflict.merge_base[..7.min(conflict.merge_base.len())]
        );
        let parents = vec![branch_tip.as_str()];

        let _resolution_sha = git::apply_conflict_resolutions(
            &repo_path,
            &auto_tree,
            &resolutions,
            &parents,
            &resolve_msg,
            branch,
        )
        .map_err(sfn_err)?;

        db::complete_merge_conflict(&pool, conflict_id)
            .await
            .map_err(sfn_err)?;

        // Remove the conflicting commit from remaining (it was just resolved)
        let still_remaining: Vec<String> = remaining_shas
            .iter()
            .filter(|s| **s != conflict.merge_base)
            .cloned()
            .collect();

        if !still_remaining.is_empty() {
            // Continue reverting the rest
            let result = git::revert_session(&repo_path, branch, &still_remaining, revert_message)
                .map_err(sfn_err)?;

            match result {
                git::RevertResult::Success => {
                    // All remaining commits reverted successfully
                }
                git::RevertResult::Conflict {
                    conflicting_sha,
                    remaining_shas: new_remaining,
                    current_commit,
                    auto_tree: new_auto_tree,
                    conflict_files,
                    ..
                } => {
                    // Another conflict — create a new conflict record and redirect
                    let parent_sha = git::rev_parse(&repo_path, &format!("{}^", conflicting_sha))
                        .map_err(sfn_err)?;

                    let new_ctx = serde_json::json!({
                        "session_id": ctx.get("session_id"),
                        "prompt_index": ctx.get("prompt_index"),
                        "remaining_shas": new_remaining,
                        "branch": branch,
                        "revert_message": revert_message,
                    });
                    let new_ctx_str = serde_json::to_string(&new_ctx).map_err(sfn_err)?;

                    let new_conflict = db::create_merge_conflict(
                        &pool,
                        repo_db.id,
                        user.id,
                        "revert",
                        &current_commit,
                        &parent_sha,
                        &conflicting_sha,
                        new_auto_tree.as_deref(),
                        Some(&new_ctx_str),
                    )
                    .await
                    .map_err(sfn_err)?;

                    for file_path in &conflict_files {
                        db::create_conflict_file(&pool, new_conflict.id, file_path, "content")
                            .await
                            .map_err(sfn_err)?;
                    }

                    leptos_axum::redirect(&format!(
                        "/{}/{}/conflicts/{}",
                        owner, repo, new_conflict.id
                    ));
                    return Ok(());
                }
            }
        }

        // Redirect back to session/prompt detail
        let redirect_url = if let Some(session_id) = ctx.get("session_id").and_then(|v| v.as_str())
        {
            if let Some(prompt_index) = ctx.get("prompt_index").and_then(|v| v.as_i64()) {
                format!(
                    "/{}/{}/ai/{}/prompt/{}",
                    owner, repo, session_id, prompt_index
                )
            } else {
                format!("/{}/{}/ai/{}", owner, repo, session_id)
            }
        } else {
            format!("/{}/{}", owner, repo)
        };

        leptos_axum::redirect(&redirect_url);
    } else {
        // Merge: two-parent commit (existing behavior)
        let message = format!(
            "Resolve {} conflicts: {} -> {}",
            conflict.operation_type, conflict.source_ref, conflict.target_ref
        );

        let parents = vec![conflict.target_ref.as_str(), conflict.source_ref.as_str()];

        // Extract branch name from target_ref (strip refs/heads/ if present)
        let branch = conflict
            .target_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(&conflict.target_ref);

        git::apply_conflict_resolutions(
            &repo_path,
            &auto_tree,
            &resolutions,
            &parents,
            &message,
            branch,
        )
        .map_err(sfn_err)?;

        db::complete_merge_conflict(&pool, conflict_id)
            .await
            .map_err(sfn_err)?;

        // Redirect back based on context
        let redirect_url = if let Some(ref ctx) = conflict.context_json {
            if let Ok(ctx) = serde_json::from_str::<serde_json::Value>(ctx) {
                if let Some(session_id) = ctx.get("session_id").and_then(|v| v.as_str()) {
                    format!("/{}/{}/ai/{}", owner, repo, session_id)
                } else {
                    format!("/{}/{}", owner, repo)
                }
            } else {
                format!("/{}/{}", owner, repo)
            }
        } else {
            format!("/{}/{}", owner, repo)
        };

        leptos_axum::redirect(&redirect_url);
    }

    Ok(())
}

#[server]
async fn cancel_resolution(
    owner: String,
    repo: String,
    conflict_id: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{get_repo_pools, require_auth, sfn_err};
    use oxigit_core::db;

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let conflict = db::get_merge_conflict(&pool, conflict_id)
        .await
        .map_err(sfn_err)?
        .ok_or_else(|| ServerFnError::new("Conflict not found"))?;

    if conflict.repo_id != repo_db.id || conflict.user_id != user.id {
        return Err(ServerFnError::new("Access denied"));
    }

    db::cancel_merge_conflict(&pool, conflict_id)
        .await
        .map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}", owner, repo));
    Ok(())
}

#[component]
pub fn ConflictResolvePage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let conflict_id = move || {
        params
            .read()
            .get("conflict_id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let (selected_file, set_selected_file) = signal(Option::<String>::None);

    let detail = Resource::new(
        move || (owner(), repo(), conflict_id()),
        move |(o, r, id)| fetch_conflict_detail(o, r, id),
    );

    let file_content = Resource::new(
        move || (owner(), repo(), conflict_id(), selected_file.get()),
        move |(o, r, id, file)| async move {
            match file {
                Some(f) => fetch_conflict_file_content(o, r, id, f).await.ok(),
                None => None,
            }
        },
    );

    let resolve_action = ServerAction::<ResolveFile>::new();
    let complete_action = ServerAction::<CompleteResolution>::new();
    let cancel_action = ServerAction::<CancelResolution>::new();

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                let cid = conflict_id();
                Suspend::new(async move {
                    match detail.await {
                        Ok(d) => {
                            if d.status != "pending" {
                                return view! {
                                    <div class="flash flash-success">
                                        "This conflict has been " {d.status} "."
                                    </div>
                                }.into_any();
                            }

                            // Auto-select first unresolved file
                            if selected_file.get_untracked().is_none() {
                                if let Some(f) = d.files.iter().find(|f| !f.is_resolved) {
                                    set_selected_file.set(Some(f.file_path.clone()));
                                } else if let Some(f) = d.files.first() {
                                    set_selected_file.set(Some(f.file_path.clone()));
                                }
                            }

                            let file_list: Vec<AnyView> = d.files.iter().map(|f| {
                                let path = f.file_path.clone();
                                let is_selected = selected_file.get() == Some(path.clone());
                                let status_icon = if f.is_resolved { "+" } else { "o" };
                                let resolution_label = f.resolution.clone().unwrap_or_default();
                                let path_click = path.clone();
                                view! {
                                    <div
                                        class={if is_selected { "conflict-file-item selected" } else { "conflict-file-item" }}
                                        on:click=move |_| set_selected_file.set(Some(path_click.clone()))
                                        style="cursor: pointer;"
                                    >
                                        <span class={if f.is_resolved { "conflict-status resolved" } else { "conflict-status" }}>
                                            {status_icon}
                                        </span>
                                        <span class="conflict-file-path">{path}</span>
                                        {(!resolution_label.is_empty()).then(|| view! {
                                            <span class="tag">{resolution_label}</span>
                                        })}
                                    </div>
                                }.into_any()
                            }).collect();

                            let on1 = owner_name.clone();
                            let rn1 = repo_name.clone();
                            let on2 = owner_name.clone();
                            let rn2 = repo_name.clone();
                            let on3 = owner_name.clone();
                            let rn3 = repo_name.clone();

                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span>"Resolve Conflicts"</span>
                                    </h1>
                                </div>

                                <div class="card mb-4">
                                    <div style="padding: var(--space-3);">
                                        <span class="ai-badge">{d.operation_type.clone()}</span>
                                        " "
                                        <span class="text-secondary">
                                            {if d.operation_type == "revert" {
                                                format!("Reverting commit {}", &d.source_ref[..7.min(d.source_ref.len())])
                                            } else {
                                                format!("{} -> {}", d.source_ref, d.target_ref)
                                            }}
                                        </span>
                                        " — "
                                        <span class="text-secondary">
                                            {d.files.len()} " conflicting file" {if d.files.len() != 1 { "s" } else { "" }}
                                        </span>
                                    </div>
                                </div>

                                <div class="conflict-layout">
                                    // File list sidebar
                                    <div class="conflict-sidebar card">
                                        <div class="card-header">"Files"</div>
                                        <div class="conflict-file-list">{file_list}</div>
                                    </div>

                                    // File content panel
                                    <div class="conflict-content card">
                                        <Suspense fallback=|| view! { <p class="text-secondary" style="padding: var(--space-4);">"Select a file..."</p> }>
                                            {move || {
                                                let on = on1.clone();
                                                let rn = rn1.clone();
                                                Suspend::new(async move {
                                                    match file_content.await {
                                                        Some(fc) => {
                                                            // Find file id
                                                            let file_id = {
                                                                if let Ok(d) = fetch_conflict_detail(on.clone(), rn.clone(), cid).await {
                                                                    d.files.iter().find(|f| f.file_path == fc.file_path).map(|f| f.id).unwrap_or(0)
                                                                } else { 0 }
                                                            };

                                                            let ours = fc.ours_content.clone().unwrap_or_else(|| "(file does not exist)".into());
                                                            let theirs = fc.theirs_content.clone().unwrap_or_else(|| "(file does not exist)".into());
                                                            let resolved = fc.resolved_content.clone();
                                                            let ours_label_btn = fc.ours_label.clone();
                                                            let theirs_label_btn = fc.theirs_label.clone();
                                                            let ours_label_side = fc.ours_label.clone();
                                                            let theirs_label_side = fc.theirs_label;
                                                            let ours_for_editor = ours.clone();
                                                            let on_a = on.clone();
                                                            let rn_a = rn.clone();
                                                            let on_b = on.clone();
                                                            let rn_b = rn.clone();
                                                            let on_c = on;
                                                            let rn_c = rn;

                                                            view! {
                                                                <div class="card-header">
                                                                    <code>{fc.file_path.clone()}</code>
                                                                </div>

                                                                // Resolution buttons
                                                                <div class="conflict-actions">
                                                                    <ActionForm action=resolve_action>
                                                                        <input type="hidden" name="owner" value={on_a} />
                                                                        <input type="hidden" name="repo" value={rn_a} />
                                                                        <input type="hidden" name="conflict_id" value={cid.to_string()} />
                                                                        <input type="hidden" name="file_id" value={file_id.to_string()} />
                                                                        <input type="hidden" name="resolution" value="ours" />
                                                                        <button type="submit" class="btn btn-sm">
                                                                            "Accept " {ours_label_btn}
                                                                        </button>
                                                                    </ActionForm>
                                                                    <ActionForm action=resolve_action>
                                                                        <input type="hidden" name="owner" value={on_b} />
                                                                        <input type="hidden" name="repo" value={rn_b} />
                                                                        <input type="hidden" name="conflict_id" value={cid.to_string()} />
                                                                        <input type="hidden" name="file_id" value={file_id.to_string()} />
                                                                        <input type="hidden" name="resolution" value="theirs" />
                                                                        <button type="submit" class="btn btn-sm">
                                                                            "Accept " {theirs_label_btn}
                                                                        </button>
                                                                    </ActionForm>
                                                                </div>

                                                                // Side-by-side content
                                                                <div class="conflict-compare">
                                                                    <div class="conflict-side">
                                                                        <div class="conflict-side-label">{ours_label_side} " (ours)"</div>
                                                                        <pre class="conflict-code">{ours}</pre>
                                                                    </div>
                                                                    <div class="conflict-side">
                                                                        <div class="conflict-side-label">{theirs_label_side} " (theirs)"</div>
                                                                        <pre class="conflict-code">{theirs}</pre>
                                                                    </div>
                                                                </div>

                                                                // Manual edit
                                                                <details class="conflict-manual-edit">
                                                                    <summary>"Edit manually"</summary>
                                                                    <ActionForm action=resolve_action>
                                                                        <input type="hidden" name="owner" value={on_c} />
                                                                        <input type="hidden" name="repo" value={rn_c} />
                                                                        <input type="hidden" name="conflict_id" value={cid.to_string()} />
                                                                        <input type="hidden" name="file_id" value={file_id.to_string()} />
                                                                        <input type="hidden" name="resolution" value="manual" />
                                                                        <textarea name="manual_content" class="conflict-editor"
                                                                            rows="20">{resolved.unwrap_or(ours_for_editor)}</textarea>
                                                                        <button type="submit" class="btn btn-primary btn-sm mt-2">
                                                                            "Save Manual Resolution"
                                                                        </button>
                                                                    </ActionForm>
                                                                </details>
                                                            }.into_any()
                                                        }
                                                        None => view! {
                                                            <p class="text-secondary" style="padding: var(--space-4);">"Select a file to resolve."</p>
                                                        }.into_any(),
                                                    }
                                                })
                                            }}
                                        </Suspense>
                                    </div>
                                </div>

                                // Bottom actions
                                <div class="conflict-bottom-actions mt-4 mb-4">
                                    <ActionForm action=cancel_action>
                                        <input type="hidden" name="owner" value={on2} />
                                        <input type="hidden" name="repo" value={rn2} />
                                        <input type="hidden" name="conflict_id" value={cid.to_string()} />
                                        <button type="submit" class="btn btn-sm">"Cancel"</button>
                                    </ActionForm>
                                    <ActionForm action=complete_action>
                                        <input type="hidden" name="owner" value={on3} />
                                        <input type="hidden" name="repo" value={rn3} />
                                        <input type="hidden" name="conflict_id" value={cid.to_string()} />
                                        <button type="submit" class="btn btn-primary"
                                            disabled={move || !d.all_resolved}>
                                            {if d.operation_type == "revert" { "Complete Revert" } else { "Complete Merge" }}
                                        </button>
                                    </ActionForm>
                                </div>
                            }.into_any()
                        }
                        Err(e) => view! {
                            <ErrorDisplay error=e.to_string() />
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
