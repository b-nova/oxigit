use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use super::{AiMetadataInfo, CommitSummary, DiffReviewData, DiffSummaryInfo, RiskFlagInfo};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitDetail {
    pub commit: CommitSummary,
    pub diff_html: String,
    pub ai_metadata: Option<AiMetadataInfo>,
}

#[server]
async fn fetch_commit_diff(
    owner: String,
    repo: String,
    sha: String,
) -> Result<CommitDetail, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let (commit_info, diff) = git::show_commit_diff(&repo_path, &sha)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Render diff as HTML with line coloring
    let diff_html = render_diff(&diff);

    // Fetch AI metadata if available
    let ai_metadata = db::get_ai_metadata_for_commit(&pool, repo_db.id, &sha)
        .await
        .ok()
        .flatten()
        .map(|m| {
            use super::AiMetadataInfo;
            AiMetadataInfo {
                ai_tool: m.ai_tool,
                ai_model: m.ai_model,
                ai_prompt: m.ai_prompt,
                ai_session_id: m.ai_session_id,
                ai_files_touched: m.ai_files_touched.and_then(|f| serde_json::from_str(&f).ok()),
            }
        });

    Ok(CommitDetail {
        commit: CommitSummary {
            message: commit_info.message,
            author: commit_info.author,
            time: commit_info.time,
        },
        diff_html,
        ai_metadata,
    })
}

#[server]
async fn attach_ai_metadata(
    owner: String,
    repo: String,
    commit_sha: String,
    ai_tool: String,
    ai_model: Option<String>,
    ai_prompt: Option<String>,
    ai_session_id: Option<String>,
    ai_files_touched: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Check write access
    let can_push = db::can_push_repo(&pool, &repo_db, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if !can_push {
        return Err(ServerFnError::new("Access denied"));
    }

    // Convert comma-separated files to JSON array if provided
    let files_json = ai_files_touched.map(|f| {
        let files: Vec<String> = f.split(',').map(|s| s.trim().to_string()).collect();
        serde_json::to_string(&files).unwrap_or_default()
    });

    db::insert_ai_metadata(
        &pool,
        repo_db.id,
        &commit_sha,
        &ai_tool,
        ai_model.as_deref(),
        ai_prompt.as_deref(),
        ai_session_id.as_deref(),
        files_json.as_deref(),
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn get_diff_review(
    owner: String,
    repo: String,
    sha: String,
) -> Result<DiffReviewData, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool, get_effective_llm_config};
    use oxigit_core::{db, git, risk};

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;
    let (llm_provider, _, _, _) = get_effective_llm_config(current_user.as_ref().map(|u| u.id)).await?;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let (_, diff) = git::show_commit_diff(&repo_path, &sha)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
        .into_iter()
        .map(|f| RiskFlagInfo {
            category: f.category.label().to_string(),
            message: f.message,
            file: f.file,
        })
        .collect();

    let cached = db::get_diff_summary(&pool, repo_db.id, &sha)
        .await
        .ok()
        .flatten()
        .map(|s| {
            let flags: Vec<RiskFlagInfo> = s.risk_flags
                .and_then(|f| serde_json::from_str(&f).ok())
                .unwrap_or_default();
            DiffSummaryInfo {
                summary: s.summary,
                risk_flags: flags,
                generated_by: s.generated_by,
            }
        });

    Ok(DiffReviewData {
        cached_summary: cached,
        risk_flags,
        llm_available: llm_provider != "none",
    })
}

#[server]
async fn generate_diff_summary(
    owner: String,
    repo: String,
    sha: String,
) -> Result<DiffSummaryInfo, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool, get_effective_llm_config};
    use oxigit_core::{db, git, llm, risk};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let (provider, api_key, model, base_url) = get_effective_llm_config(Some(user.id)).await?;

    if provider == "none" {
        return Err(ServerFnError::new("LLM not configured"));
    }

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, Some(user.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let (_, diff) = git::show_commit_diff(&repo_path, &sha)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Get AI prompt context if available
    let ai_prompt = db::get_ai_metadata_for_commit(&pool, repo_db.id, &sha)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.ai_prompt);

    let config = llm::LlmConfig { provider, api_key, model: model.clone(), base_url };
    let summary = llm::generate_summary(&config, &diff, ai_prompt.as_deref())
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
        .into_iter()
        .map(|f| RiskFlagInfo {
            category: f.category.label().to_string(),
            message: f.message,
            file: f.file,
        })
        .collect();

    let flags_json = serde_json::to_string(&risk_flags).ok();

    db::upsert_diff_summary(&pool, repo_db.id, &sha, &summary, flags_json.as_deref(), &model)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(DiffSummaryInfo {
        summary,
        risk_flags,
        generated_by: model,
    })
}

#[cfg(feature = "ssr")]
fn render_diff(diff: &str) -> String {
    use std::fmt::Write;
    let mut html = String::new();
    let mut in_file = false;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            if in_file {
                html.push_str("</pre></div>");
            }
            in_file = true;
            let _ = write!(
                html,
                r#"<div class="diff-file"><div class="diff-header">{}</div><pre class="diff-content">"#,
                escape_html(line)
            );
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

    if in_file {
        html.push_str("</pre></div>");
    }

    html
}

#[cfg(feature = "ssr")]
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[component]
pub fn CommitViewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let sha = move || params.read().get("sha").unwrap_or_default();

    let detail = Resource::new(
        move || (owner(), repo(), sha()),
        move |(owner, repo, sha)| fetch_commit_diff(owner, repo, sha),
    );

    let review = Resource::new(
        move || (owner(), repo(), sha()),
        move |(owner, repo, sha)| get_diff_review(owner, repo, sha),
    );

    let generate_action = ServerAction::<GenerateDiffSummary>::new();

    view! {
        <Suspense fallback=|| view! { <p class="text-secondary mt-8">"Loading..."</p> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                let commit_sha = sha();
                Suspend::new(async move {
                    match detail.await {
                        Ok(d) => {
                            let short_sha = &commit_sha[..7.min(commit_sha.len())];
                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <a href={format!("/{}/{}/commits", owner_name, repo_name)}>"commits"</a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span class="commit-sha">{short_sha.to_string()}</span>
                                    </h1>
                                </div>
                                <div class="card mb-4">
                                    <div class="commit-message" style="font-size: 1.125rem; margin-bottom: var(--space-2);">
                                        {d.commit.message.clone()}
                                    </div>
                                    <div class="commit-meta">
                                        {d.commit.author.clone()} " committed " {d.commit.time.clone()}
                                    </div>
                                </div>
                                {d.ai_metadata.clone().map(|meta| {
                                    let session_href = meta.ai_session_id.clone().map(|sid| {
                                        format!("/{}/{}/ai-timeline?session={}", owner_name, repo_name, sid)
                                    });
                                    view! {
                                        <div class="ai-context-panel mb-4">
                                            <div class="ai-context-header">
                                                <span class="ai-badge">{meta.ai_tool.clone()}</span>
                                                {meta.ai_model.clone().map(|model| view! {
                                                    <span class="ai-model-tag">{model}</span>
                                                })}
                                                {session_href.map(|href| view! {
                                                    <a href={href} class="ai-session-link">"View session"</a>
                                                })}
                                            </div>
                                            {meta.ai_prompt.clone().map(|prompt| view! {
                                                <details class="ai-prompt-details">
                                                    <summary>"Prompt"</summary>
                                                    <div class="ai-prompt-bubble">{prompt}</div>
                                                </details>
                                            })}
                                            {meta.ai_files_touched.clone().map(|files| view! {
                                                <div class="ai-files-touched">
                                                    <span class="text-secondary">"AI-touched files: "</span>
                                                    {files.join(", ")}
                                                </div>
                                            })}
                                        </div>
                                    }
                                })}
                                // AI Diff Review panel
                                <Suspense fallback=|| ()>
                                    {move || {
                                        let on = owner_name.clone();
                                        let rn = repo_name.clone();
                                        let cs = commit_sha.clone();
                                        let gen_value = generate_action.value();
                                        Suspend::new(async move {
                                            let review_data = review.await.ok();
                                            let has_risk = review_data.as_ref().map_or(false, |r| !r.risk_flags.is_empty());
                                            let has_summary = review_data.as_ref().map_or(false, |r| r.cached_summary.is_some());
                                            let llm_available = review_data.as_ref().map_or(false, |r| r.llm_available);
                                            let show_panel = has_risk || has_summary || llm_available;

                                            if !show_panel {
                                                return view! { <div></div> }.into_any();
                                            }

                                            let risk_flags = review_data.as_ref().map(|r| r.risk_flags.clone()).unwrap_or_default();
                                            let cached = review_data.and_then(|r| r.cached_summary);

                                            view! {
                                                <div class="ai-review-panel mb-4">
                                                    <div class="ai-review-header">
                                                        <span class="ai-review-title">"AI Diff Review"</span>
                                                    </div>
                                                    // Risk badges
                                                    {(!risk_flags.is_empty()).then(|| {
                                                        let flags = risk_flags.clone();
                                                        view! {
                                                            <div class="risk-badges">
                                                                {flags.into_iter().map(|f| {
                                                                    let class = format!("risk-badge risk-{}", f.category);
                                                                    let label = format!("{}: {}", f.category, f.message);
                                                                    view! {
                                                                        <span class={class} title={f.file.unwrap_or_default()}>
                                                                            {label}
                                                                        </span>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </div>
                                                        }
                                                    })}
                                                    // Cached summary
                                                    {cached.map(|s| view! {
                                                        <div class="ai-review-summary">{s.summary}</div>
                                                        <div class="ai-review-meta">
                                                            "Generated by " {s.generated_by}
                                                        </div>
                                                    })}
                                                    // Generate button (from action result)
                                                    {move || {
                                                        let generated = gen_value.get().and_then(|r| r.ok());
                                                        generated.map(|s| view! {
                                                            <div class="ai-review-summary">{s.summary}</div>
                                                            <div class="ai-review-meta">
                                                                "Generated by " {s.generated_by}
                                                            </div>
                                                        })
                                                    }}
                                                    // Generate button (if no cached and LLM available)
                                                    {(llm_available && !has_summary).then(|| {
                                                        let gen_on = on.clone();
                                                        let gen_rn = rn.clone();
                                                        let gen_cs = cs.clone();
                                                        view! {
                                                            <ActionForm action=generate_action>
                                                                <input type="hidden" name="owner" value={gen_on} />
                                                                <input type="hidden" name="repo" value={gen_rn} />
                                                                <input type="hidden" name="sha" value={gen_cs} />
                                                                <button type="submit" class="btn btn-sm btn-generate">
                                                                    "Generate AI Summary"
                                                                </button>
                                                            </ActionForm>
                                                        }
                                                    })}
                                                </div>
                                            }.into_any()
                                        })
                                    }}
                                </Suspense>
                                <div class="diff-container" inner_html={d.diff_html}></div>
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
