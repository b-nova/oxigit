use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use super::{AiMetadataInfo, AiTimelineEntry, SessionListItem, SessionListResponse};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiHubResponse {
    pub sessions: Vec<SessionListItem>,
    pub unsessioned: Vec<AiTimelineEntry>,
}

#[server]
async fn fetch_ai_hub(
    owner: String,
    repo: String,
) -> Result<AiHubResponse, ServerFnError> {
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

    // Sessions
    let summaries = db::list_session_summaries(&pool, repo_db.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let sessions = summaries
        .into_iter()
        .map(|s| SessionListItem {
            session_id: s.session_id,
            ai_tool: s.ai_tool,
            commit_count: s.commit_count,
            first_prompt: s.first_prompt,
            first_time: s.first_time,
            last_time: s.last_time,
        })
        .collect();

    // Unsessioned commits
    let unsessioned_metas = db::get_unsessioned_ai_commits(&pool, repo_db.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let unsessioned = unsessioned_metas
        .into_iter()
        .map(|meta| {
            let commit_info = git::get_latest_commit(&repo_path, &meta.commit_sha).unwrap_or(None);
            let (message, author, time) = match commit_info {
                Some(c) => (c.message, c.author, c.time),
                None => ("(commit not found)".into(), String::new(), String::new()),
            };
            AiTimelineEntry {
                short_sha: meta.commit_sha[..7.min(meta.commit_sha.len())].to_string(),
                commit_sha: meta.commit_sha,
                commit_message: message,
                commit_author: author,
                commit_time: time,
                metadata: AiMetadataInfo {
                    ai_tool: meta.ai_tool,
                    ai_model: meta.ai_model,
                    ai_prompt: meta.ai_prompt,
                    ai_session_id: None,
                    ai_files_touched: meta.ai_files_touched.and_then(|f| serde_json::from_str(&f).ok()),
                },
            }
        })
        .collect();

    Ok(AiHubResponse { sessions, unsessioned })
}

#[component]
pub fn AiHubPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let hub = Resource::new(
        move || (owner(), repo()),
        move |(owner, repo)| fetch_ai_hub(owner, repo),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"AI"</span>
            </h1>
        </div>
        <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match hub.await {
                        Ok(resp) if resp.sessions.is_empty() && resp.unsessioned.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">"No AI activity yet."</p>
                                <p class="empty-state-text">
                                    "Push commits with AI metadata to see sessions and conversation history here."
                                </p>
                            </div>
                        }.into_any(),
                        Ok(resp) => {
                            let mut parts: Vec<AnyView> = Vec::new();

                            // Sessions
                            if !resp.sessions.is_empty() {
                                let cards: Vec<AnyView> = resp.sessions.into_iter().map(|s| {
                                    let href = format!("/{}/{}/ai/{}", owner_name, repo_name, s.session_id);
                                    let short_id = s.session_id[..8.min(s.session_id.len())].to_string();
                                    let prompt_preview = s.first_prompt
                                        .map(|p| if p.len() > 100 { format!("{}...", &p[..100]) } else { p })
                                        .unwrap_or_default();
                                    view! {
                                        <a href={href} class="session-card">
                                            <div class="session-card-header">
                                                <span class="ai-badge">{s.ai_tool}</span>
                                                <span class="session-card-id">{short_id}</span>
                                                <span class="text-tertiary">
                                                    {s.commit_count} " commit" {if s.commit_count != 1 { "s" } else { "" }}
                                                </span>
                                            </div>
                                            {(!prompt_preview.is_empty()).then(|| view! {
                                                <p class="session-card-prompt">{prompt_preview}</p>
                                            })}
                                            <div class="session-card-time">
                                                {s.first_time} " — " {s.last_time}
                                            </div>
                                        </a>
                                    }.into_any()
                                }).collect();
                                parts.push(view! {
                                    <div class="session-list mb-4">{cards}</div>
                                }.into_any());
                            }

                            // Unsessioned commits
                            if !resp.unsessioned.is_empty() {
                                let items: Vec<AnyView> = resp.unsessioned.into_iter().map(|entry| {
                                    let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, entry.commit_sha);
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="flex-row gap-2">
                                                    <span class="ai-badge">{entry.metadata.ai_tool}</span>
                                                    <a href={commit_href} class="commit-sha">{entry.short_sha}</a>
                                                    <span>{entry.commit_message}</span>
                                                </div>
                                                {entry.metadata.ai_prompt.map(|p| view! {
                                                    <p class="list-item-desc">{p}</p>
                                                })}
                                            </div>
                                            <span class="list-item-meta">{entry.commit_time}</span>
                                        </li>
                                    }.into_any()
                                }).collect();
                                parts.push(view! {
                                    <div class="card-flush">
                                        <div class="card-header">"Individual AI commits"</div>
                                        <ul class="list">{items}</ul>
                                    </div>
                                }.into_any());
                            }

                            view! { <div>{parts}</div> }.into_any()
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
