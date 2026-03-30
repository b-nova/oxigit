use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[allow(unused_imports)]
use super::{AiMetadataInfo, AiTimelineEntry, DiffSummaryInfo, RiskFlagInfo, SessionDetailResponse};

#[server]
async fn fetch_session_detail(
    owner: String,
    repo: String,
    session_id: String,
) -> Result<SessionDetailResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool, get_effective_llm_config};
    use oxigit_core::{db, git, llm, risk};

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

    let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &session_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Session not found"));
    }

    let ai_tool = metas[0].ai_tool.clone();
    let ai_model = metas[0].ai_model.clone();

    // Build entries and collect files (metas are ordered by created_at ASC)
    let mut entries = Vec::new();
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
                ai_files_touched: meta.ai_files_touched.as_ref().and_then(|f| serde_json::from_str(f).ok()),
            },
        });
    }

    let first_time = entries.first().map(|e| e.commit_time.clone()).unwrap_or_default();
    let last_time = entries.last().map(|e| e.commit_time.clone()).unwrap_or_default();

    // Aggregate diff
    let diff = git::session_aggregate_diff(&repo_path, &shas).unwrap_or_default();
    let diff_html = render_diff(&diff);

    // Auto-generate AI summary if LLM configured
    let cache_key = format!("session-{}", session_id);
    let cached_summary = db::get_diff_summary(&pool, repo_db.id, &cache_key).await.ok().flatten();

    let summary = if let Some(s) = cached_summary {
        let flags: Vec<RiskFlagInfo> = s.risk_flags
            .and_then(|f| serde_json::from_str(&f).ok())
            .unwrap_or_default();
        Some(DiffSummaryInfo {
            summary: s.summary,
            risk_flags: flags,
            generated_by: s.generated_by,
        })
    } else {
        let (provider, api_key, model, base_url) = get_effective_llm_config(current_user.as_ref().map(|u| u.id)).await?;
        if provider != "none" && !diff.is_empty() {
            let first_prompt = entries.iter().find_map(|e| e.metadata.ai_prompt.clone());
            let config = llm::LlmConfig { provider, api_key, model: model.clone(), base_url };
            match llm::generate_summary(&config, &diff, first_prompt.as_deref()).await {
                Ok(summary_text) => {
                    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
                        .into_iter()
                        .map(|f| RiskFlagInfo { category: f.category.label().to_string(), message: f.message, file: f.file })
                        .collect();
                    let flags_json = serde_json::to_string(&risk_flags).ok();
                    let _ = db::upsert_diff_summary(&pool, repo_db.id, &cache_key, &summary_text, flags_json.as_deref(), &model).await;
                    Some(DiffSummaryInfo { summary: summary_text, risk_flags, generated_by: model })
                }
                Err(_) => None,
            }
        } else {
            None
        }
    };

    // Check revert capability
    let can_revert = current_user
        .as_ref()
        .map(|u| u.id == repo_db.owner_id)
        .unwrap_or(false);

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
    })
}

#[server]
async fn revert_session(
    owner: String,
    repo: String,
    session_id: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let can_push = db::can_push_repo(&pool, &repo_db, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if !can_push {
        return Err(ServerFnError::new("Access denied"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &session_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if metas.is_empty() {
        return Err(ServerFnError::new("Session not found"));
    }

    let shas: Vec<String> = metas.iter().map(|m| m.commit_sha.clone()).collect();
    let first_prompt = metas.iter().find_map(|m| m.ai_prompt.clone()).unwrap_or_default();
    let short_id = &session_id[..8.min(session_id.len())];

    let default_branch = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    let message = format!("Revert AI session {}: {}", short_id, first_prompt);
    git::revert_session(&repo_path, &default_branch, &shas, &message)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    leptos_axum::redirect(&format!("/{}/{}/sessions/{}", owner, repo, session_id));
    Ok(())
}

#[cfg(feature = "ssr")]
fn render_diff(diff: &str) -> String {
    use std::fmt::Write;
    let mut html = String::new();
    let mut in_file = false;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            if in_file { html.push_str("</pre></div>"); }
            in_file = true;
            let _ = write!(html, r#"<div class="diff-file"><div class="diff-header">{}</div><pre class="diff-content">"#, escape_html(line));
        } else if line.starts_with("+++") || line.starts_with("---") {
            let _ = write!(html, r#"<span class="diff-meta">{}</span>"#, escape_html(line));
            html.push('\n');
        } else if line.starts_with("@@") {
            let _ = write!(html, r#"<span class="diff-hunk">{}</span>"#, escape_html(line));
            html.push('\n');
        } else if line.starts_with('+') {
            let _ = write!(html, r#"<span class="diff-add">{}</span>"#, escape_html(line));
            html.push('\n');
        } else if line.starts_with('-') {
            let _ = write!(html, r#"<span class="diff-del">{}</span>"#, escape_html(line));
            html.push('\n');
        } else {
            let _ = write!(html, "{}", escape_html(line));
            html.push('\n');
        }
    }
    if in_file { html.push_str("</pre></div>"); }
    html
}

#[cfg(feature = "ssr")]
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[component]
pub fn SessionViewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let session_id = move || params.read().get("session_id").unwrap_or_default();

    let detail = Resource::new(
        move || (owner(), repo(), session_id()),
        move |(o, r, s)| fetch_session_detail(o, r, s),
    );

    let revert_action = ServerAction::<RevertSession>::new();

    view! {
        <Suspense fallback=|| view! { <p class="text-secondary mt-8">"Loading..."</p> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match detail.await {
                        Ok(d) => {
                            let short_id = d.session_id[..8.min(d.session_id.len())].to_string();
                            let commit_count = d.entries.len();
                            let sid = d.session_id.clone();

                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <a href={format!("/{}/{}/sessions", owner_name, repo_name)}>"Sessions"</a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span>{short_id.clone()}</span>
                                    </h1>
                                </div>

                                // Session header
                                <div class="session-header card mb-4">
                                    <div class="session-header-row">
                                        <span class="ai-badge">{d.ai_tool.clone()}</span>
                                        {d.ai_model.clone().map(|m| view! {
                                            <span class="ai-model-tag">{m}</span>
                                        })}
                                        <span class="text-secondary">
                                            {commit_count} " commit" {if commit_count != 1 { "s" } else { "" }}
                                        </span>
                                    </div>
                                    <div class="session-header-time text-tertiary">
                                        {d.first_time.clone()} " — " {d.last_time.clone()}
                                    </div>
                                </div>

                                // AI Summary
                                {d.summary.map(|s| view! {
                                    <div class="ai-review-panel mb-4">
                                        <div class="ai-review-header">
                                            <span class="ai-review-title">"Session Summary"</span>
                                        </div>
                                        <div class="ai-review-summary">{s.summary}</div>
                                        <div class="ai-review-meta">"Generated by " {s.generated_by}</div>
                                    </div>
                                })}

                                // Files changed
                                {(!d.files_changed.is_empty()).then(|| {
                                    let files = d.files_changed.clone();
                                    let count = files.len();
                                    view! {
                                        <div class="card mb-4">
                                            <div class="card-header">{count} " file(s) changed"</div>
                                            <div class="session-files">
                                                {files.into_iter().map(|f| view! {
                                                    <code class="session-file">{f}</code>
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </div>
                                    }
                                })}

                                // Timeline
                                <div class="card-flush mb-4">
                                    <div class="card-header">"Timeline"</div>
                                    <div class="ai-session-entries" style="padding: var(--space-3) var(--space-4);">
                                        {d.entries.into_iter().map(|entry| {
                                            let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, entry.commit_sha);
                                            view! {
                                                <div class="ai-timeline-entry">
                                                    <div class="ai-timeline-connector"></div>
                                                    <div class="ai-timeline-content">
                                                        {entry.metadata.ai_prompt.map(|prompt| view! {
                                                            <div class="ai-prompt-bubble">{prompt}</div>
                                                        })}
                                                        <div class="ai-timeline-commit">
                                                            <a href={commit_href} class="commit-sha">{entry.short_sha}</a>
                                                            " "
                                                            <span>{entry.commit_message}</span>
                                                            <span class="text-tertiary ml-2">
                                                                {entry.commit_author} " " {entry.commit_time}
                                                            </span>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>

                                // Revert button
                                {d.can_revert.then(|| {
                                    let on = owner_name.clone();
                                    let rn = repo_name.clone();
                                    let s = sid.clone();
                                    view! {
                                        <div class="card mb-4">
                                            <ActionForm action=revert_action>
                                                <input type="hidden" name="owner" value={on} />
                                                <input type="hidden" name="repo" value={rn} />
                                                <input type="hidden" name="session_id" value={s} />
                                                <button type="submit" class="btn btn-danger">
                                                    "Revert Session"
                                                </button>
                                                <span class="text-secondary" style="margin-left: var(--space-3); font-size: 0.8125rem;">
                                                    "Creates a single revert commit undoing all changes from this session."
                                                </span>
                                            </ActionForm>
                                        </div>
                                    }
                                })}

                                // Aggregate diff
                                {(!d.diff_html.is_empty()).then(|| {
                                    let dh = d.diff_html.clone();
                                    view! {
                                        <h3 style="margin-bottom: var(--space-3);">"Aggregate Diff"</h3>
                                        <div class="diff-container" inner_html={dh}></div>
                                    }
                                })}
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
