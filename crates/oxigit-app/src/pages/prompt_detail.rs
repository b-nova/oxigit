use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[allow(unused_imports)]
use super::{AiTimelineEntry, PromptDetailResponse};

#[server]
async fn fetch_prompt_detail(
    owner: String,
    repo: String,
    session_id: String,
    prompt_index: i64,
) -> Result<PromptDetailResponse, ServerFnError> {
    use super::{AiMetadataInfo, DiffSummaryInfo, RiskFlagInfo, VibeScoreInfo};
    use crate::server_fns::{
        extract_session_user, get_effective_llm_config, get_repo_path, get_repo_pools,
        get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git, llm, risk};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

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

    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Prompt not found"));
    }

    let ai_tool = metas[0].ai_tool.clone();
    let ai_model = metas[0].ai_model.clone();
    let prompt_text = metas[0].ai_prompt.clone();

    let mut commits = Vec::new();
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

        let commit_diff = git::show_commit_diff(&repo_path, &meta.commit_sha)
            .ok()
            .map(|(_, d)| super::render_diff(&d));

        shas.push(meta.commit_sha.clone());
        commits.push(AiTimelineEntry {
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

    // Time range (commits are newest-first)
    let first_time = commits
        .last()
        .map(|e| e.commit_time.clone())
        .unwrap_or_default();
    let last_time = commits
        .first()
        .map(|e| e.commit_time.clone())
        .unwrap_or_default();

    // Reverse SHAs to oldest-first for aggregate operations
    let mut shas_asc = shas.clone();
    shas_asc.reverse();

    let diff = git::session_aggregate_diff(&repo_path, &shas_asc).unwrap_or_default();
    let diff_html = super::render_diff(&diff);

    // AI summary with caching
    let cache_key = format!("prompt-{}-{}", session_id, prompt_index);
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
            let config = llm::LlmConfig {
                provider,
                api_key,
                model: model.clone(),
                base_url,
            };
            match llm::generate_summary(&config, &diff, prompt_text.as_deref()).await {
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

    // Vibe score
    let vibe_score = {
        use oxigit_core::vibe;

        let (lines_added, lines_deleted) =
            git::session_diff_stats(&repo_path, &shas_asc).unwrap_or((0, 0));
        let risk_count = summary.as_ref().map(|s| s.risk_flags.len()).unwrap_or(0);

        let short_id = &session_id[..8.min(session_id.len())];
        let grep_pattern = format!("Revert prompt #{} in session {}", prompt_index, short_id);
        let was_reverted = std::process::Command::new("git")
            .args(["log", "--all", "--grep", &grep_pattern, "--format=%H"])
            .current_dir(&repo_path)
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        let metrics = vibe::SessionMetrics {
            commit_count: commits.len(),
            prompt_count: 1,
            risk_flag_count: risk_count,
            files_touched: all_files.len(),
            lines_added,
            lines_deleted,
            was_reverted,
        };
        let vs = vibe::compute_vibe_score(&metrics);
        Some(VibeScoreInfo {
            score: vs.score,
            grade: vs.grade.to_string(),
            commits_per_prompt: vs.factors.commits_per_prompt,
            risk_density: vs.factors.risk_density,
            churn_ratio: vs.factors.churn_ratio,
            file_scope: vs.factors.file_scope,
            was_reverted,
        })
    };

    let can_operate = current_user.id == repo_db.owner_id;

    let branches = git::list_branches(&repo_path).unwrap_or_default();
    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    Ok(PromptDetailResponse {
        session_id,
        prompt_index,
        prompt_text,
        ai_tool,
        ai_model,
        commits,
        diff_html,
        files_changed: all_files,
        can_operate,
        branches,
        default_branch,
        summary,
        vibe_score,
        first_time,
        last_time,
    })
}

#[server]
async fn revert_prompt(
    owner: String,
    repo: String,
    session_id: String,
    prompt_index: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{
        extract_session_user, get_repo_path, get_repo_pools, get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

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
    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Prompt not found"));
    }

    let shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    let prompt_text = metas[0].ai_prompt.clone().unwrap_or_default();
    let short_id = &session_id[..8.min(session_id.len())];

    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    let message = format!(
        "Revert prompt #{} in session {}: {}",
        prompt_index, short_id, prompt_text
    );
    let result =
        git::revert_session(&repo_path, &default_branch, &shas, &message).map_err(sfn_err)?;

    match result {
        git::RevertResult::Success => {
            leptos_axum::redirect(&format!(
                "/{}/{}/ai/{}/prompt/{}",
                owner, repo, session_id, prompt_index
            ));
        }
        git::RevertResult::Conflict {
            conflicting_sha,
            remaining_shas,
            current_commit,
            auto_tree,
            conflict_files,
            ..
        } => {
            let parent_sha =
                git::rev_parse(&repo_path, &format!("{}^", conflicting_sha)).map_err(sfn_err)?;

            let context = serde_json::json!({
                "session_id": session_id,
                "prompt_index": prompt_index,
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
async fn cherry_pick_prompt(
    owner: String,
    repo: String,
    session_id: String,
    prompt_index: i64,
    target_branch: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{
        extract_session_user, get_repo_path, get_repo_pools, get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

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
    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Prompt not found"));
    }

    let shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();

    git::cherry_pick_range(&repo_path, &target_branch, &shas).map_err(sfn_err)?;

    leptos_axum::redirect(&format!(
        "/{}/{}/ai/{}/prompt/{}",
        owner, repo, session_id, prompt_index
    ));
    Ok(())
}

#[server]
async fn squash_prompt(
    owner: String,
    repo: String,
    session_id: String,
    prompt_index: i64,
    message: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{
        extract_session_user, get_repo_path, get_repo_pools, get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

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
    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await
        .map_err(sfn_err)?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Prompt not found"));
    }

    let mut shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    shas.reverse();

    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    git::squash_session(&repo_path, &default_branch, &shas, &message).map_err(sfn_err)?;

    leptos_axum::redirect(&format!(
        "/{}/{}/ai/{}/prompt/{}",
        owner, repo, session_id, prompt_index
    ));
    Ok(())
}

#[component]
pub fn PromptDetailPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let session_id = move || params.read().get("session_id").unwrap_or_default();
    let prompt_index = move || {
        params
            .read()
            .get("prompt_index")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let data = Resource::new(
        move || (owner(), repo(), session_id(), prompt_index()),
        move |(o, r, s, p)| fetch_prompt_detail(o, r, s, p),
    );

    let revert_action = ServerAction::<RevertPrompt>::new();
    let squash_action = ServerAction::<SquashPrompt>::new();
    let cherry_pick_action = ServerAction::<CherryPickPrompt>::new();

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match data.await {
                        Ok(d) => {
                            let short_sid = d.session_id[..8.min(d.session_id.len())].to_string();
                            let commit_count = d.commits.len();

                            let prompt_display = d.prompt_text.clone()
                                .unwrap_or_else(|| "(no prompt recorded)".to_string());

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
                                        <a href={format!("/{}/{}/ai/{}", owner_name, repo_name, d.session_id)}>
                                            {short_sid}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span>"Prompt #" {d.prompt_index.to_string()}</span>
                                    </h1>
                                </div>
                            }.into_any());

                            // Header card with vibe score, prompt text, and time range
                            parts.push(view! {
                                <div class="session-header card mb-4" style="border-left: 3px solid var(--accent);">
                                    <div class="session-header-row">
                                        {d.vibe_score.as_ref().map(|vs| {
                                            let grade_class = format!("vibe-badge vibe-{}", vs.grade);
                                            view! {
                                                <span class={grade_class}>
                                                    {vs.score.to_string()} " " {vs.grade.clone()}
                                                </span>
                                            }
                                        })}
                                        <span class="ai-badge">{d.ai_tool}</span>
                                        {d.ai_model.map(|m| view! { <span class="ai-model-tag">{m}</span> })}
                                        <span class="text-secondary">
                                            {commit_count} " commit" {if commit_count != 1 { "s" } else { "" }}
                                        </span>
                                    </div>
                                    <div class="prompt-card-text" style="font-size: 1.1rem; margin: var(--space-2) 0;">
                                        {prompt_display.clone()}
                                    </div>
                                    <div class="session-header-time text-tertiary">
                                        {d.first_time.clone()} " — " {d.last_time.clone()}
                                    </div>
                                </div>
                            }.into_any());

                            // Summary panel
                            if let Some(ref s) = d.summary {
                                let summary_text = s.summary.clone();
                                let generated_by = s.generated_by.clone();
                                parts.push(view! {
                                    <div class="ai-review-panel mb-4">
                                        <div class="ai-review-header">
                                            <span class="ai-review-title">"Prompt Summary"</span>
                                        </div>
                                        <div class="ai-review-summary">{summary_text}</div>
                                        <div class="ai-review-meta">"Generated by " {generated_by}</div>
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

                            // Commit list
                            let commit_views: Vec<AnyView> = d.commits.iter().map(|c| {
                                let href = format!("/{}/{}/commit/{}", owner_name, repo_name, c.commit_sha);
                                let diff_html = c.diff_html.clone();
                                view! {
                                    <div class="conversation-turn">
                                        <div class="turn-response">
                                            <div class="turn-commit-line">
                                                <a href={href} class="commit-sha">{c.short_sha.clone()}</a>
                                                <span>{c.commit_message.clone()}</span>
                                                <span class="text-tertiary" style="margin-left: auto; font-size: 0.75rem;">
                                                    {c.commit_time.clone()}
                                                </span>
                                            </div>
                                        </div>
                                        {diff_html.map(|dh| view! {
                                            <details class="turn-diff-toggle">
                                                <summary>"Show diff"</summary>
                                                <div class="diff-container" inner_html={dh}></div>
                                            </details>
                                        })}
                                    </div>
                                }.into_any()
                            }).collect();

                            parts.push(view! {
                                <div class="card mb-4">
                                    <div class="card-header">"Commits"</div>
                                    <div style="padding: var(--space-4);">{commit_views}</div>
                                </div>
                            }.into_any());

                            // Aggregate diff
                            parts.push(view! {
                                <div class="card mb-4">
                                    <details>
                                        <summary class="card-header" style="cursor: pointer;">"Aggregate Diff"</summary>
                                        <div class="diff-container" inner_html={d.diff_html}></div>
                                    </details>
                                </div>
                            }.into_any());

                            // Operations
                            if d.can_operate {
                                let on1 = owner_name.clone();
                                let rn1 = repo_name.clone();
                                let s1 = d.session_id.clone();
                                let p1 = d.prompt_index;
                                let on2 = owner_name.clone();
                                let rn2 = repo_name.clone();
                                let s2 = d.session_id.clone();
                                let p2 = d.prompt_index;
                                let on3 = owner_name.clone();
                                let rn3 = repo_name.clone();
                                let s3 = d.session_id.clone();
                                let p3 = d.prompt_index;

                                let branch_options: Vec<AnyView> = d.branches.iter()
                                    .filter(|b| **b != d.default_branch)
                                    .map(|b| {
                                        view! { <option value={b.clone()}>{b.clone()}</option> }.into_any()
                                    })
                                    .collect();

                                parts.push(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Prompt Operations"</div>
                                        <div class="session-ops">
                                            // Revert
                                            <div class="session-op">
                                                <ActionForm action=revert_action>
                                                    <input type="hidden" name="owner" value={on1} />
                                                    <input type="hidden" name="repo" value={rn1} />
                                                    <input type="hidden" name="session_id" value={s1} />
                                                    <input type="hidden" name="prompt_index" value={p1.to_string()} />
                                                    <button type="submit" class="btn btn-danger btn-sm">
                                                        "Revert Prompt"
                                                    </button>
                                                    <span class="session-op-desc">
                                                        "Undo this prompt's changes with a revert commit."
                                                    </span>
                                                </ActionForm>
                                            </div>
                                            // Squash
                                            <div class="session-op">
                                                <ActionForm action=squash_action>
                                                    <input type="hidden" name="owner" value={on2} />
                                                    <input type="hidden" name="repo" value={rn2} />
                                                    <input type="hidden" name="session_id" value={s2} />
                                                    <input type="hidden" name="prompt_index" value={p2.to_string()} />
                                                    <div class="session-op-row">
                                                        <button type="submit" class="btn btn-primary btn-sm">
                                                            "Squash Prompt"
                                                        </button>
                                                        <input type="text" name="message" class="form-input form-input-sm"
                                                            value={prompt_display}
                                                            placeholder="Squash commit message"
                                                            style="flex: 1;" />
                                                    </div>
                                                    <span class="session-op-desc">
                                                        "Collapse this prompt's commits into a single commit."
                                                    </span>
                                                </ActionForm>
                                            </div>
                                            // Cherry-pick
                                            <div class="session-op">
                                                <ActionForm action=cherry_pick_action>
                                                    <input type="hidden" name="owner" value={on3} />
                                                    <input type="hidden" name="repo" value={rn3} />
                                                    <input type="hidden" name="session_id" value={s3} />
                                                    <input type="hidden" name="prompt_index" value={p3.to_string()} />
                                                    <div class="session-op-row">
                                                        <button type="submit" class="btn btn-sm">
                                                            "Cherry-pick to"
                                                        </button>
                                                        <select name="target_branch" class="form-input form-input-sm">
                                                            {branch_options}
                                                        </select>
                                                    </div>
                                                    <span class="session-op-desc">
                                                        "Apply this prompt's changes to another branch."
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
