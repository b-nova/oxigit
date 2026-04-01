use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[allow(unused_imports)]
use super::{AiTimelineEntry, PromptDetailResponse};

#[server]
async fn fetch_prompt_detail(
    owner: String,
    repo: String,
    session_id: String,
    prompt_index: i64,
) -> Result<PromptDetailResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};
    use super::AiMetadataInfo;

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.as_ref().map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);

    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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

        if let Some(ref files_json) = meta.ai_files_touched {
            if let Ok(files) = serde_json::from_str::<Vec<String>>(files_json) {
                for f in &files {
                    if !all_files.contains(f) {
                        all_files.push(f.clone());
                    }
                }
            }
        }

        let commit_diff = git::show_commit_diff(&repo_path, &meta.commit_sha)
            .ok()
            .map(|(_, d)| crate::pages::ai_session_detail::render_diff_public(&d));

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
                ai_files_touched: meta.ai_files_touched.as_ref().and_then(|f| serde_json::from_str(f).ok()),
            },
            diff_html: commit_diff,
        });
    }

    let diff = git::session_aggregate_diff(&repo_path, &shas).unwrap_or_default();
    let diff_html = crate::pages::ai_session_detail::render_diff_public(&diff);

    let can_operate = current_user.as_ref().map(|u| {
        u.id == repo_db.owner_id
    }).unwrap_or(false);

    let branches = git::list_branches(&repo_path).unwrap_or_default();
    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None).unwrap_or_else(|| "main".to_string());

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
    })
}

#[server]
async fn revert_prompt(
    owner: String,
    repo: String,
    session_id: String,
    prompt_index: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let can_push = db::can_push_repo(&pool, &repo_db, user.id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;
    if !can_push {
        return Err(ServerFnError::new("Access denied"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Prompt not found"));
    }

    let shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    let prompt_text = metas[0].ai_prompt.clone().unwrap_or_default();
    let short_id = &session_id[..8.min(session_id.len())];

    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None).unwrap_or_else(|| "main".to_string());

    let message = format!("Revert prompt #{} in session {}: {}", prompt_index, short_id, prompt_text);
    git::revert_session(&repo_path, &default_branch, &shas, &message)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    leptos_axum::redirect(&format!("/{}/{}/ai/{}/prompt/{}", owner, repo, session_id, prompt_index));
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
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let can_push = db::can_push_repo(&pool, &repo_db, user.id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;
    if !can_push {
        return Err(ServerFnError::new("Access denied"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let metas = db::get_commits_for_prompt_group(&pool, repo_db.id, &session_id, prompt_index)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Prompt not found"));
    }

    let shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();

    git::cherry_pick_range(&repo_path, &target_branch, &shas)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    leptos_axum::redirect(&format!("/{}/{}/ai/{}/prompt/{}", owner, repo, session_id, prompt_index));
    Ok(())
}

#[component]
pub fn PromptDetailPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let session_id = move || params.read().get("session_id").unwrap_or_default();
    let prompt_index = move || {
        params.read().get("prompt_index")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let data = Resource::new(
        move || (owner(), repo(), session_id(), prompt_index()),
        move |(o, r, s, p)| fetch_prompt_detail(o, r, s, p),
    );

    let revert_action = ServerAction::<RevertPrompt>::new();
    let cherry_pick_action = ServerAction::<CherryPickPrompt>::new();

    view! {
        <Suspense fallback=|| view! { <p class="text-secondary mt-8">"Loading..."</p> }>
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

                            // Files
                            let files_view = if d.files_changed.is_empty() {
                                None
                            } else {
                                let files: Vec<AnyView> = d.files_changed.iter().map(|f| {
                                    view! { <code class="session-file">{f.clone()}</code> }.into_any()
                                }).collect();
                                Some(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">{d.files_changed.len()} " file(s) changed"</div>
                                        <div class="session-files">{files}</div>
                                    </div>
                                })
                            };

                            // Operations
                            let ops_view = if d.can_operate {
                                let on1 = owner_name.clone();
                                let rn1 = repo_name.clone();
                                let s1 = d.session_id.clone();
                                let p1 = d.prompt_index;
                                let on2 = owner_name.clone();
                                let rn2 = repo_name.clone();
                                let s2 = d.session_id.clone();
                                let p2 = d.prompt_index;

                                let branch_options: Vec<AnyView> = d.branches.iter()
                                    .filter(|b| **b != d.default_branch)
                                    .map(|b| {
                                        view! { <option value={b.clone()}>{b.clone()}</option> }.into_any()
                                    })
                                    .collect();

                                Some(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Prompt Operations"</div>
                                        <div class="session-ops">
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
                                            <div class="session-op">
                                                <ActionForm action=cherry_pick_action>
                                                    <input type="hidden" name="owner" value={on2} />
                                                    <input type="hidden" name="repo" value={rn2} />
                                                    <input type="hidden" name="session_id" value={s2} />
                                                    <input type="hidden" name="prompt_index" value={p2.to_string()} />
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
                                })
                            } else {
                                None
                            };

                            view! {
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

                                // Prompt card
                                <div class="card mb-4" style="border-left: 3px solid var(--accent);">
                                    <div style="padding: var(--space-4);">
                                        <div class="prompt-card-text" style="font-size: 1.1rem; margin-bottom: var(--space-2);">
                                            {prompt_display}
                                        </div>
                                        <div class="prompt-card-meta">
                                            <span class="ai-badge">{d.ai_tool}</span>
                                            {d.ai_model.map(|m| view! { <span class="tag">{m}</span> })}
                                            <span class="text-tertiary">
                                                {commit_count} " commit" {if commit_count != 1 { "s" } else { "" }}
                                            </span>
                                        </div>
                                    </div>
                                </div>

                                {files_view}

                                // Commits
                                <div class="card mb-4">
                                    <div class="card-header">"Commits"</div>
                                    <div style="padding: var(--space-4);">{commit_views}</div>
                                </div>

                                // Aggregate diff
                                <div class="card mb-4">
                                    <details>
                                        <summary class="card-header" style="cursor: pointer;">"Aggregate Diff"</summary>
                                        <div class="diff-container" inner_html={d.diff_html}></div>
                                    </details>
                                </div>

                                {ops_view}
                            }.into_any()
                        }
                        Err(e) => view! {
                            <div class="flash flash-error">{e.to_string()}</div>
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
