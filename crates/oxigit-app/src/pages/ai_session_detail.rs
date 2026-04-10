use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;
use crate::components::toast::use_toast;

#[allow(unused_imports)]
use super::{
    AiMetadataInfo, AiTimelineEntry, DiffSummaryInfo, RiskFlagInfo, SessionDetailResponse,
};

#[server]
async fn fetch_session_detail(
    owner: String,
    repo: String,
    session_id: String,
) -> Result<SessionDetailResponse, ServerFnError> {
    use crate::server_fns::{
        require_auth, get_effective_llm_config, get_repo_path, get_repo_pools,
        get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git, llm, risk};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = require_auth().await?;

    let entitlements = get_user_entitlements(current_user.id).await?;
    if !entitlements.ai_features {
        return Err(ServerFnError::new(
            "AI features require a Flat or higher plan. Upgrade at /pricing",
        ));
    }

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, Some(current_user.id)) {
        return Err(ServerFnError::new("Repository not found"));
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

    let mut entries = Vec::new();
    let mut all_files: Vec<String> = Vec::new();
    let mut shas: Vec<String> = Vec::new();

    for meta in &metas {
        let commit_info = git::get_latest_commit(&repo_path, &meta.commit_sha).unwrap_or(None);
        let (message, author, time) = match commit_info {
            Some(c) => (c.message, c.author, c.time),
            None => ("(commit not found)".into(), String::new(), String::new()),
        };

        if let Some(ref files_json) = meta.ai_files_touched
            && let Ok(files) = serde_json::from_str::<Vec<String>>(files_json)
        {
            for f in &files {
                if !all_files.contains(f) {
                    all_files.push(f.clone());
                }
            }
        }

        // Get per-commit diff
        let commit_diff = git::show_commit_diff(&repo_path, &meta.commit_sha)
            .ok()
            .map(|(_, d)| super::render_diff(&d));

        shas.push(meta.commit_sha.clone());
        entries.push(AiTimelineEntry {
            short_sha: meta.commit_sha[..7.min(meta.commit_sha.len())].to_string(),
            commit_sha: meta.commit_sha.clone(),
            commit_message: message,
            commit_author: author,
            commit_time: time,
            metadata: AiMetadataInfo {
                ai_tool: meta.ai_tool.clone(),
                ai_model: meta.ai_model.clone(),
                ai_prompt: meta.ai_prompt.clone(),
                ai_session_id: meta.ai_session_id.clone(),
                ai_files_touched: meta
                    .ai_files_touched
                    .as_ref()
                    .and_then(|f| serde_json::from_str(f).ok()),
                ai_prompt_index: meta.ai_prompt_index,
            },
            diff_html: commit_diff,
        });
    }

    // Entries are newest-first; times: first entry is latest, last is earliest
    let first_time = entries
        .last()
        .map(|e| e.commit_time.clone())
        .unwrap_or_default();
    let last_time = entries
        .first()
        .map(|e| e.commit_time.clone())
        .unwrap_or_default();

    // Aggregate diff needs SHAs in oldest-first order
    let mut shas_asc = shas.clone();
    shas_asc.reverse();
    let diff = git::session_aggregate_diff(&repo_path, &shas_asc).unwrap_or_default();
    let diff_html = super::render_diff(&diff);

    // Auto-generate AI summary if LLM configured
    let cache_key = format!("session-{}", session_id);
    let cached_summary = db::get_diff_summary(&pool, repo_db.id, &cache_key)
        .await
        .ok()
        .flatten();

    let summary = if let Some(s) = cached_summary {
        let flags: Vec<RiskFlagInfo> = s
            .risk_flags
            .and_then(|f| serde_json::from_str(&f).ok())
            .unwrap_or_default();
        Some(DiffSummaryInfo {
            summary: s.summary,
            risk_flags: flags,
            generated_by: s.generated_by,
        })
    } else {
        let (provider, api_key, model, base_url) =
            get_effective_llm_config(Some(current_user.id)).await?;
        if provider != "none" && !diff.is_empty() {
            let first_prompt = entries.iter().find_map(|e| e.metadata.ai_prompt.clone());
            let config = llm::LlmConfig {
                provider,
                api_key,
                model: model.clone(),
                base_url,
            };
            match llm::generate_summary(&config, &diff, first_prompt.as_deref()).await {
                Ok(summary_text) => {
                    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
                        .into_iter()
                        .map(|f| RiskFlagInfo {
                            category: f.category.label().to_string(),
                            message: f.message,
                            file: f.file,
                        })
                        .collect();
                    let flags_json = serde_json::to_string(&risk_flags).ok();
                    let _ = db::upsert_diff_summary(
                        &pool,
                        repo_db.id,
                        &cache_key,
                        &summary_text,
                        flags_json.as_deref(),
                        &model,
                    )
                    .await;
                    Some(DiffSummaryInfo {
                        summary: summary_text,
                        risk_flags,
                        generated_by: model,
                    })
                }
                Err(_) => None,
            }
        } else {
            None
        }
    };

    let can_revert = current_user.id == repo_db.owner_id;
    let branches = git::list_branches(&repo_path).unwrap_or_default();
    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    // Compute vibe score
    let vibe_score = {
        use oxigit_core::vibe;

        let (lines_added, lines_deleted) =
            git::session_diff_stats(&repo_path, &shas_asc).unwrap_or((0, 0));
        let prompt_count = entries
            .iter()
            .filter_map(|e| e.metadata.ai_prompt.as_ref())
            .collect::<std::collections::HashSet<_>>()
            .len()
            .max(1);
        let risk_count = summary.as_ref().map(|s| s.risk_flags.len()).unwrap_or(0);
        let was_reverted = git::is_session_reverted(&repo_path, &session_id);

        let metrics = vibe::SessionMetrics {
            commit_count: entries.len(),
            prompt_count,
            risk_flag_count: risk_count,
            files_touched: all_files.len(),
            lines_added,
            lines_deleted,
            was_reverted,
        };
        let vs = vibe::compute_vibe_score(&metrics);
        Some(super::VibeScoreInfo {
            score: vs.score,
            grade: vs.grade.to_string(),
            commits_per_prompt: vs.factors.commits_per_prompt,
            risk_density: vs.factors.risk_density,
            churn_ratio: vs.factors.churn_ratio,
            file_scope: vs.factors.file_scope,
            was_reverted,
        })
    };

    let recipe_id = db::get_recipe_for_session(&pool, repo_db.id, &session_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.id);

    Ok(SessionDetailResponse {
        session_id,
        ai_tool,
        ai_model,
        entries,
        diff_html,
        files_changed: all_files,
        first_time,
        last_time,
        summary,
        can_revert,
        branches,
        default_branch,
        vibe_score,
        recipe_id,
    })
}

#[server]
async fn revert_session(
    owner: String,
    repo: String,
    session_id: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{
        require_auth, get_repo_path, get_repo_pools, get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git};

    let user = require_auth().await?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.ai_features {
        return Err(ServerFnError::new(
            "AI features require a Flat or higher plan. Upgrade at /pricing",
        ));
    }

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

    let repo_path = get_repo_path(&owner, &repo).await?;
    let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &session_id)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Session not found"));
    }

    // Metas are DESC — reverse for revert (needs oldest-first)
    let mut shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    shas.reverse();
    let first_prompt = metas
        .last()
        .and_then(|m| m.ai_prompt.clone())
        .unwrap_or_default();
    let short_id = &session_id[..8.min(session_id.len())];

    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    let message = format!("Revert AI session {}: {}", short_id, first_prompt);
    let result =
        git::revert_session(&repo_path, &default_branch, &shas, &message).map_err(sfn_err)?;

    match result {
        git::RevertResult::Success => {
            leptos_axum::redirect(&format!("/{}/{}/ai/{}", owner, repo, session_id));
        }
        git::RevertResult::Conflict {
            conflicting_sha,
            remaining_shas,
            current_commit,
            auto_tree,
            conflict_files,
            ..
        } => {
            // Get parent of conflicting commit (the revert target state)
            let parent_sha =
                git::rev_parse(&repo_path, &format!("{}^", conflicting_sha)).map_err(sfn_err)?;

            let context = serde_json::json!({
                "session_id": session_id,
                "prompt_index": serde_json::Value::Null,
                "remaining_shas": remaining_shas,
                "branch": default_branch,
                "revert_message": message,
            });
            let context_str = serde_json::to_string(&context).map_err(sfn_err)?;

            let conflict = db::create_merge_conflict(
                &pool,
                repo_db.id,
                user.id,
                "revert",
                &current_commit,
                &parent_sha,
                &conflicting_sha,
                auto_tree.as_deref(),
                Some(&context_str),
            )
            .await
            .map_err(sfn_err)?;

            for file_path in &conflict_files {
                db::create_conflict_file(&pool, conflict.id, file_path, "content")
                    .await
                    .map_err(sfn_err)?;
            }

            leptos_axum::redirect(&format!("/{}/{}/conflicts/{}", owner, repo, conflict.id));
        }
    }

    Ok(())
}

#[server]
async fn squash_session_action(
    owner: String,
    repo: String,
    session_id: String,
    message: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{
        require_auth, get_repo_path, get_repo_pools, get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git};

    let user = require_auth().await?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.ai_features {
        return Err(ServerFnError::new(
            "AI features require a Flat or higher plan. Upgrade at /pricing",
        ));
    }

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

    let repo_path = get_repo_path(&owner, &repo).await?;
    let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &session_id)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Session not found"));
    }

    // Metas are DESC — reverse for oldest-first
    let mut shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    shas.reverse();

    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    git::squash_session(&repo_path, &default_branch, &shas, &message).map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/ai/{}", owner, repo, session_id));
    Ok(())
}

#[server]
async fn cherry_pick_session_action(
    owner: String,
    repo: String,
    session_id: String,
    target_branch: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{
        require_auth, get_repo_path, get_repo_pools, get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git};

    let user = require_auth().await?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.ai_features {
        return Err(ServerFnError::new(
            "AI features require a Flat or higher plan. Upgrade at /pricing",
        ));
    }

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

    let repo_path = get_repo_path(&owner, &repo).await?;
    let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &session_id)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Session not found"));
    }

    // Metas are DESC — reverse for oldest-first
    let mut shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    shas.reverse();

    git::cherry_pick_range(&repo_path, &target_branch, &shas).map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/ai/{}", owner, repo, session_id));
    Ok(())
}

#[component]
pub fn AiSessionDetailPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let session_id = move || params.read().get("session_id").unwrap_or_default();

    let detail = Resource::new(
        move || (owner(), repo(), session_id()),
        move |(o, r, s)| fetch_session_detail(o, r, s),
    );

    let revert_action = ServerAction::<RevertSession>::new();
    let squash_action = ServerAction::<SquashSessionAction>::new();
    let cherry_pick_action = ServerAction::<CherryPickSessionAction>::new();

    let toast = use_toast();
    let toast_revert = toast.clone();
    let toast_squash = toast.clone();
    let toast_cherry = toast.clone();
    Effect::new(move |_| {
        if let Some(Ok(_)) = revert_action.value().get() {
            toast_revert.success("Session reverted");
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(_)) = squash_action.value().get() {
            toast_squash.success("Session squashed");
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(_)) = cherry_pick_action.value().get() {
            toast_cherry.success("Session cherry-picked");
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match detail.await {
                        Ok(d) => {
                            let short_id = d.session_id[..8.min(d.session_id.len())].to_string();
                            let commit_count = d.entries.len();
                            let sid = d.session_id.clone();

                            let mut parts: Vec<AnyView> = Vec::new();

                            // Breadcrumb
                            parts.push(view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <a href={format!("/{}/{}/ai", owner_name, repo_name)}>"AI"</a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span>{short_id.clone()}</span>
                                    </h1>
                                </div>
                            }.into_any());

                            // Header
                            parts.push(view! {
                                <div class="session-header card mb-4">
                                    <div class="session-header-row">
                                        {d.vibe_score.as_ref().map(|vs| {
                                            let grade_class = format!("vibe-badge vibe-{}", vs.grade);
                                            view! {
                                                <span class={grade_class}>
                                                    {vs.score.to_string()} " " {vs.grade.clone()}
                                                </span>
                                            }
                                        })}
                                        <span class="ai-badge">{d.ai_tool.clone()}</span>
                                        {d.ai_model.clone().map(|m| view! { <span class="ai-model-tag">{m}</span> })}
                                        <span class="text-secondary">
                                            {commit_count} " commit" {if commit_count != 1 { "s" } else { "" }}
                                        </span>
                                    </div>
                                    <div class="session-header-time text-tertiary">
                                        {d.first_time.clone()} " — " {d.last_time.clone()}
                                    </div>
                                </div>
                            }.into_any());

                            // Summary
                            if let Some(s) = d.summary {
                                parts.push(view! {
                                    <div class="ai-review-panel mb-4">
                                        <div class="ai-review-header">
                                            <span class="ai-review-title">"Session Summary"</span>
                                        </div>
                                        <div class="ai-review-summary">{s.summary}</div>
                                        <div class="ai-review-meta">"Generated by " {s.generated_by}</div>
                                    </div>
                                }.into_any());
                            }

                            // Files changed
                            if !d.files_changed.is_empty() {
                                let files: Vec<AnyView> = d.files_changed.iter().map(|f| {
                                    view! { <code class="session-file">{f.clone()}</code> }.into_any()
                                }).collect();
                                parts.push(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">{d.files_changed.len()} " file(s) changed"</div>
                                        <div class="session-files">{files}</div>
                                    </div>
                                }.into_any());
                            }

                            // Group entries by prompt_index for drill-down
                            // Entries are newest-first; collect into groups preserving order
                            let mut prompt_groups: Vec<(Option<i64>, String, Vec<&AiTimelineEntry>)> = Vec::new();
                            for entry in &d.entries {
                                let idx = entry.metadata.ai_prompt_index;
                                let prompt_text = entry.metadata.ai_prompt.clone()
                                    .unwrap_or_else(|| entry.commit_message.clone());
                                if let Some(group) = prompt_groups.iter_mut().find(|(gi, _, _)| *gi == idx) {
                                    group.2.push(entry);
                                } else {
                                    prompt_groups.push((idx, prompt_text, vec![entry]));
                                }
                            }
                            // Reverse so oldest prompt group is first
                            prompt_groups.reverse();

                            let prompt_cards: Vec<AnyView> = prompt_groups.iter().enumerate().map(|(display_idx, (prompt_idx, prompt_text, group_entries))| {
                                let prompt_index_val = prompt_idx.unwrap_or(display_idx as i64);
                                let prompt_href = format!("/{}/{}/ai/{}/prompt/{}", owner_name, repo_name, sid, prompt_index_val);

                                let turn_items: Vec<AnyView> = group_entries.iter().map(|entry| {
                                    let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, entry.commit_sha);
                                    let files = entry.metadata.ai_files_touched.clone().unwrap_or_default();
                                    let file_count = files.len();
                                    let files_str = files.join(", ");
                                    let entry_diff = entry.diff_html.clone();

                                    view! {
                                        <div class="conversation-turn">
                                            <div class="turn-response">
                                                <div class="turn-commit-line">
                                                    <a href={commit_href} class="commit-sha">{entry.short_sha.clone()}</a>
                                                    <span>{entry.commit_message.clone()}</span>
                                                    <span class="text-tertiary" style="margin-left: auto; font-size: 0.75rem;">{entry.commit_time.clone()}</span>
                                                </div>
                                                {(!files_str.is_empty()).then(|| view! {
                                                    <div class="turn-files">{files_str}</div>
                                                })}
                                            </div>
                                            {entry_diff.map(|dh| {
                                                let summary_text = if file_count > 0 {
                                                    format!("Show generated code ({} file{})", file_count, if file_count != 1 { "s" } else { "" })
                                                } else {
                                                    "Show generated code".to_string()
                                                };
                                                view! {
                                                    <details class="turn-diff-toggle">
                                                        <summary>{summary_text}</summary>
                                                        <div class="diff-container" inner_html={dh}></div>
                                                    </details>
                                                }
                                            })}
                                        </div>
                                    }.into_any()
                                }).collect();

                                let commit_count = group_entries.len();

                                view! {
                                    <div class="card mb-4" style="border-left: 3px solid var(--accent);">
                                        <div class="card-header" style="display: flex; align-items: center; gap: var(--space-2);">
                                            <a href={prompt_href} style="text-decoration: none; color: inherit; display: flex; align-items: center; gap: var(--space-2); flex: 1;">
                                                <span class="text-secondary" style="font-size: 0.8rem; font-weight: 600;">"Prompt #" {prompt_index_val.to_string()}</span>
                                                <span style="flex: 1;">{prompt_text.clone()}</span>
                                                <span class="text-tertiary" style="font-size: 0.75rem;">
                                                    {commit_count} " commit" {if commit_count != 1 { "s" } else { "" }}
                                                    " →"
                                                </span>
                                            </a>
                                        </div>
                                        <div style="padding: var(--space-4);">{turn_items}</div>
                                    </div>
                                }.into_any()
                            }).collect();

                            parts.push(view! {
                                <div class="mb-4">
                                    <h2 class="text-secondary mb-2" style="font-size: 1rem;">"Prompts"</h2>
                                    {prompt_cards}
                                </div>
                            }.into_any());

                            // Session operations
                            if d.can_revert {
                                let on1 = owner_name.clone();
                                let rn1 = repo_name.clone();
                                let s1 = sid.clone();
                                let on2 = owner_name.clone();
                                let rn2 = repo_name.clone();
                                let s2 = sid.clone();
                                let on3 = owner_name.clone();
                                let rn3 = repo_name.clone();
                                let s3 = sid.clone();

                                let first_prompt = d.entries.iter()
                                    .rev()
                                    .find_map(|e| e.metadata.ai_prompt.clone())
                                    .unwrap_or_else(|| "Squash AI session".to_string());

                                let branch_options: Vec<AnyView> = d.branches.iter()
                                    .filter(|b| **b != d.default_branch)
                                    .map(|b| {
                                        view! { <option value={b.clone()}>{b.clone()}</option> }.into_any()
                                    })
                                    .collect();

                                // Share / View Recipe button
                                if let Some(rid) = d.recipe_id {
                                    parts.push(view! {
                                        <a href={format!("/recipes/{}", rid)} class="btn btn-sm mb-4">
                                            "View Recipe"
                                        </a>
                                    }.into_any());
                                } else {
                                    let share_href = format!("/{}/{}/ai/{}/share", owner_name, repo_name, sid);
                                    parts.push(view! {
                                        <a href={share_href} class="btn btn-primary btn-sm mb-4">
                                            "Share as Recipe"
                                        </a>
                                    }.into_any());
                                }

                                parts.push(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Session Operations"</div>
                                        <div class="session-ops">
                                            // Revert
                                            <div class="session-op">
                                                <ActionForm action=revert_action>
                                                    <input type="hidden" name="owner" value={on1} />
                                                    <input type="hidden" name="repo" value={rn1} />
                                                    <input type="hidden" name="session_id" value={s1} />
                                                    <button type="submit" class="btn btn-danger btn-sm">
                                                        "Revert Session"
                                                    </button>
                                                    <span class="session-op-desc">
                                                        "Undo all session changes with a revert commit."
                                                    </span>
                                                </ActionForm>
                                            </div>
                                            // Squash
                                            <div class="session-op">
                                                <ActionForm action=squash_action>
                                                    <input type="hidden" name="owner" value={on2} />
                                                    <input type="hidden" name="repo" value={rn2} />
                                                    <input type="hidden" name="session_id" value={s2} />
                                                    <div class="session-op-row">
                                                        <button type="submit" class="btn btn-primary btn-sm">
                                                            "Squash Session"
                                                        </button>
                                                        <input type="text" name="message" class="form-input form-input-sm"
                                                            value={first_prompt}
                                                            placeholder="Squash commit message"
                                                            style="flex: 1;" />
                                                    </div>
                                                    <span class="session-op-desc">
                                                        "Collapse all session commits into a single commit."
                                                    </span>
                                                </ActionForm>
                                            </div>
                                            // Cherry-pick
                                            <div class="session-op">
                                                <ActionForm action=cherry_pick_action>
                                                    <input type="hidden" name="owner" value={on3} />
                                                    <input type="hidden" name="repo" value={rn3} />
                                                    <input type="hidden" name="session_id" value={s3} />
                                                    <div class="session-op-row">
                                                        <button type="submit" class="btn btn-sm">
                                                            "Cherry-pick to"
                                                        </button>
                                                        <select name="target_branch" class="form-input form-input-sm">
                                                            {branch_options}
                                                        </select>
                                                    </div>
                                                    <span class="session-op-desc">
                                                        "Apply this session's changes to another branch."
                                                    </span>
                                                </ActionForm>
                                            </div>
                                        </div>
                                    </div>
                                }.into_any());
                            }

                            view! { <div>{parts}</div> }.into_any()
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
