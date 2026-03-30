use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use super::{AiMetadataInfo, CommitSummary};

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
