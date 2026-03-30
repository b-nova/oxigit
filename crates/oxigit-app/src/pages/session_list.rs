use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[allow(unused_imports)]
use super::{SessionListItem, SessionListResponse};

#[server]
async fn fetch_session_list(
    owner: String,
    repo: String,
) -> Result<SessionListResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let pool = get_pool().await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

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

    Ok(SessionListResponse { sessions })
}

#[component]
pub fn SessionListPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let sessions = Resource::new(
        move || (owner(), repo()),
        move |(owner, repo)| fetch_session_list(owner, repo),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Sessions"</span>
            </h1>
        </div>
        <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match sessions.await {
                        Ok(resp) if resp.sessions.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">"No AI sessions yet."</p>
                                <p class="empty-state-text">
                                    "Push commits with an AI session ID to see them here."
                                </p>
                            </div>
                        }.into_any(),
                        Ok(resp) => view! {
                            <div class="session-list">
                                {resp.sessions.into_iter().map(|s| {
                                    let href = format!("/{}/{}/sessions/{}", owner_name, repo_name, s.session_id);
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
                                            {(!prompt_preview.is_empty()).then(|| {
                                                view! {
                                                    <p class="session-card-prompt">{prompt_preview}</p>
                                                }
                                            })}
                                            <div class="session-card-time">
                                                {s.first_time} " — " {s.last_time}
                                            </div>
                                        </a>
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
