use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_params_map};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[allow(unused_imports)]
use super::{BlameLineInfo, BlameResponse};

#[server]
async fn fetch_blame(
    owner: String,
    repo: String,
    path: String,
    git_ref: String,
) -> Result<BlameResponse, ServerFnError> {
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

    let git_ref = if git_ref.is_empty() {
        git::default_branch(&repo_path)
            .unwrap_or(None)
            .unwrap_or_else(|| "main".to_string())
    } else {
        git_ref
    };

    let blame_lines = git::blame_file(&repo_path, &git_ref, &path)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Collect unique SHAs and batch-fetch AI metadata
    let unique_shas: Vec<String> = {
        let mut shas: Vec<String> = blame_lines.iter().map(|l| l.commit_sha.clone()).collect();
        shas.sort();
        shas.dedup();
        shas
    };

    let ai_map = db::get_ai_metadata_for_commits_map(&pool, repo_db.id, &unique_shas)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut ai_line_count = 0;
    let lines: Vec<BlameLineInfo> = blame_lines.iter().map(|bl| {
        let ai_meta = ai_map.get(&bl.commit_sha);
        let is_ai = ai_meta.is_some();
        if is_ai { ai_line_count += 1; }

        BlameLineInfo {
            line_number: bl.line_number,
            content: bl.content.clone(),
            commit_sha: bl.commit_sha.clone(),
            short_sha: bl.commit_sha[..7.min(bl.commit_sha.len())].to_string(),
            author: bl.author.clone(),
            time: bl.time.clone(),
            is_ai,
            ai_tool: ai_meta.map(|m| m.ai_tool.clone()),
            ai_prompt: ai_meta.and_then(|m| m.ai_prompt.clone()),
            ai_session_id: ai_meta.and_then(|m| m.ai_session_id.clone()),
            ai_prompt_index: ai_meta.and_then(|m| m.ai_prompt_index),
        }
    }).collect();

    let total_line_count = lines.len();
    let file_name = path.rsplit('/').next().unwrap_or(&path).to_string();

    Ok(BlameResponse {
        file_path: path,
        file_name,
        lines,
        ai_line_count,
        total_line_count,
    })
}

#[component]
pub fn BlamePage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let path = move || params.read().get("path").unwrap_or_default();

    let location = use_location();
    let git_ref = move || {
        let search = location.search.get();
        search
            .strip_prefix('?')
            .unwrap_or(&search)
            .split('&')
            .find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k == "ref").then(|| v.to_string())
            })
            .unwrap_or_default()
    };

    let data = Resource::new(
        move || (owner(), repo(), path(), git_ref()),
        move |(o, r, p, g)| fetch_blame(o, r, p, g),
    );

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                let current_ref = git_ref();
                Suspend::new(async move {
                    match data.await {
                        Ok(resp) => {
                            let ai_pct = if resp.total_line_count > 0 {
                                (resp.ai_line_count as f64 / resp.total_line_count as f64 * 100.0) as u32
                            } else { 0 };

                            let blob_href = {
                                let ref_param = if current_ref.is_empty() { String::new() } else { format!("?ref={}", current_ref) };
                                format!("/{}/{}/blob/{}{}", owner_name, repo_name, resp.file_path, ref_param)
                            };

                            let blame_lines: Vec<AnyView> = resp.lines.iter().map(|line| {
                                let row_class = if line.is_ai { "blame-row blame-ai" } else { "blame-row" };
                                let session_link = line.ai_session_id.as_ref().map(|sid| {
                                    format!("/{}/{}/ai/{}", owner_name, repo_name, sid)
                                });
                                let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, line.commit_sha);

                                view! {
                                    <tr class={row_class}>
                                        <td class="blame-line-no">{line.line_number}</td>
                                        <td class="blame-gutter">
                                            <a href={commit_href} class="blame-sha">{line.short_sha.clone()}</a>
                                            <span class="blame-author">{line.author.clone()}</span>
                                        </td>
                                        <td class="blame-ai-col">
                                            {line.is_ai.then(|| {
                                                let tool = line.ai_tool.clone().unwrap_or_default();
                                                let href = session_link.clone();
                                                view! {
                                                    {match href {
                                                        Some(h) => view! { <a href={h} class="blame-ai-badge">{tool}</a> }.into_any(),
                                                        None => view! { <span class="blame-ai-badge">{tool}</span> }.into_any(),
                                                    }}
                                                }
                                            })}
                                        </td>
                                        <td class="blame-code"><pre>{line.content.clone()}</pre></td>
                                    </tr>
                                }.into_any()
                            }).collect();

                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span class="text-secondary">{resp.file_path.clone()}</span>
                                    </h1>
                                    <a href={blob_href} class="btn btn-sm">"Source"</a>
                                </div>

                                // Stats bar
                                <div class="blame-stats card mb-4">
                                    <div class="blame-stats-inner">
                                        {if resp.ai_line_count > 0 {
                                            view! {
                                                <div class="blame-stat">
                                                    <span class="blame-stat-value blame-stat-ai">{ai_pct} "%"</span>
                                                    <span class="blame-stat-label">"AI-generated"</span>
                                                </div>
                                                <div class="blame-stat">
                                                    <span class="blame-stat-value">{resp.ai_line_count}</span>
                                                    <span class="blame-stat-label">"AI lines"</span>
                                                </div>
                                                <div class="blame-stat">
                                                    <span class="blame-stat-value">{resp.total_line_count - resp.ai_line_count}</span>
                                                    <span class="blame-stat-label">"human lines"</span>
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <span class="text-secondary">"No AI-generated lines detected in this file."</span>
                                            }.into_any()
                                        }}
                                        <div class="blame-stat" style="margin-left: auto;">
                                            <span class="blame-stat-value">{resp.total_line_count}</span>
                                            <span class="blame-stat-label">"total lines"</span>
                                        </div>
                                    </div>
                                    // AI percentage bar
                                    {(resp.ai_line_count > 0).then(|| view! {
                                        <div class="blame-bar">
                                            <div class="blame-bar-fill" style={format!("width: {}%", ai_pct)}></div>
                                        </div>
                                    })}
                                </div>

                                // Blame table
                                <div class="card-flush">
                                    <table class="blame-table">
                                        <tbody>{blame_lines}</tbody>
                                    </table>
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
