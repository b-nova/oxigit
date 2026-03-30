use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[allow(unused_imports)]
use super::{AiMetadataInfo, AiTimelineEntry, AiTimelineResponse, AiTimelineSession};

#[server]
async fn fetch_ai_timeline(
    owner: String,
    repo: String,
) -> Result<AiTimelineResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};
    use std::collections::BTreeMap;

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
    let all_metadata = db::list_ai_sessions(&pool, repo_db.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Group by session_id
    let mut session_map: BTreeMap<Option<String>, Vec<AiTimelineEntry>> = BTreeMap::new();
    let mut session_tools: BTreeMap<Option<String>, String> = BTreeMap::new();

    for meta in all_metadata {
        let commit_info = git::get_latest_commit(&repo_path, &meta.commit_sha)
            .unwrap_or(None);

        let (message, author, time) = match commit_info {
            Some(c) => (c.message, c.author, c.time),
            None => ("(commit not found)".to_string(), String::new(), String::new()),
        };

        let entry = AiTimelineEntry {
            short_sha: meta.commit_sha[..7.min(meta.commit_sha.len())].to_string(),
            commit_sha: meta.commit_sha,
            commit_message: message,
            commit_author: author,
            commit_time: time,
            metadata: AiMetadataInfo {
                ai_tool: meta.ai_tool.clone(),
                ai_model: meta.ai_model,
                ai_prompt: meta.ai_prompt,
                ai_session_id: meta.ai_session_id.clone(),
                ai_files_touched: meta.ai_files_touched.and_then(|f| serde_json::from_str(&f).ok()),
            },
        };

        let key = meta.ai_session_id.clone();
        session_tools.entry(key.clone()).or_insert(meta.ai_tool);
        session_map.entry(key).or_default().push(entry);
    }

    let sessions: Vec<AiTimelineSession> = session_map
        .into_iter()
        .map(|(session_id, entries)| {
            let ai_tool = session_tools
                .get(&session_id)
                .cloned()
                .unwrap_or_default();
            AiTimelineSession {
                session_id,
                ai_tool,
                entries,
            }
        })
        .collect();

    Ok(AiTimelineResponse {
        owner,
        repo,
        sessions,
    })
}

#[component]
pub fn AiTimelinePage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let timeline = Resource::new(
        move || (owner(), repo()),
        move |(owner, repo)| fetch_ai_timeline(owner, repo),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"AI Timeline"</span>
            </h1>
        </div>
        <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match timeline.await {
                        Ok(resp) if resp.sessions.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">"No AI-generated commits yet."</p>
                                <p class="empty-state-text">
                                    "Push commits with AI trailers (AI-Tool, AI-Model, AI-Prompt) to see them here."
                                </p>
                            </div>
                        }.into_any(),
                        Ok(resp) => view! {
                            <div class="ai-timeline">
                                {resp.sessions.into_iter().map(|session| {
                                    let session_label = session.session_id
                                        .as_ref()
                                        .map(|s| format!("Session {}", &s[..8.min(s.len())]))
                                        .unwrap_or_else(|| "Individual commits".to_string());
                                    let tool = session.ai_tool.clone();
                                    let owner_c = owner_name.clone();
                                    let repo_c = repo_name.clone();
                                    view! {
                                        <div class="ai-session-group">
                                            <div class="ai-session-header">
                                                <span class="ai-badge">{tool}</span>
                                                <span class="ai-session-label">{session_label}</span>
                                                <span class="text-tertiary">
                                                    {format!("{} commit{}", session.entries.len(), if session.entries.len() == 1 { "" } else { "s" })}
                                                </span>
                                            </div>
                                            <div class="ai-session-entries">
                                                {session.entries.into_iter().map(|entry| {
                                                    let commit_href = format!("/{}/{}/commit/{}", owner_c, repo_c, entry.commit_sha);
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
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any(),
                        Err(e) => view! {
                            <div class="flash flash-error">{e.to_string()}</div>
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
