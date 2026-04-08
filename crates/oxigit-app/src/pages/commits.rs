use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingCard;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitEntry {
    pub id: String,
    pub short_id: String,
    pub message: String,
    pub author: String,
    pub time: String,
    pub ai_tool: Option<String>,
}

#[server]
pub async fn fetch_commits(
    owner: String,
    repo: String,
) -> Result<Vec<CommitEntry>, ServerFnError> {
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
    let git_ref = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    let raw_commits = git::list_commits(&repo_path, &git_ref, 50).unwrap_or_default();
    let shas: Vec<String> = raw_commits.iter().map(|c| c.id.clone()).collect();
    let ai_metas = db::get_ai_metadata_for_commits(&pool, repo_db.id, &shas)
        .await
        .unwrap_or_default();
    let ai_map: std::collections::HashMap<String, String> = ai_metas
        .into_iter()
        .map(|m| (m.commit_sha, m.ai_tool))
        .collect();

    let commits = raw_commits
        .into_iter()
        .map(|c| {
            let ai_tool = ai_map.get(&c.id).cloned();
            CommitEntry {
                short_id: c.id[..7.min(c.id.len())].to_string(),
                id: c.id,
                message: c.message,
                author: c.author,
                time: c.time,
                ai_tool,
            }
        })
        .collect();

    Ok(commits)
}

#[component]
pub fn CommitsPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let commits = Resource::new(
        move || (owner(), repo()),
        move |(owner, repo)| fetch_commits(owner, repo),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Commits"</span>
            </h1>
        </div>
        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
                {move || {
                    let owner_name = owner();
                    let repo_name = repo();
                    Suspend::new(async move {
                        match commits.await {
                            Ok(commits) if commits.is_empty() => view! {
                                <div class="empty-state">"No commits yet."</div>
                            }.into_any(),
                            Ok(commits) => view! {
                                <ul class="list">
                                    {commits.into_iter().map(|c| {
                                        let sha = c.id.clone();
                                        let short = c.short_id.clone();
                                        let msg = c.message.clone();
                                        let author = c.author.clone();
                                        let time = c.time.clone();
                                        let ai_tool = c.ai_tool.clone();
                                        let href = format!("/{}/{}/commit/{}", owner_name, repo_name, sha);
                                        view! {
                                            <li class="commit-item">
                                                <div class="flex-1">
                                                    <div class="commit-message">
                                                        <a href={href} style="color: var(--text);">{msg}</a>
                                                        {ai_tool.map(|tool| view! {
                                                            <span class="ai-badge ai-badge-sm">{tool}</span>
                                                        })}
                                                    </div>
                                                    <div class="commit-meta">
                                                        {author} " committed " {time}
                                                    </div>
                                                </div>
                                                <span class="commit-sha">{short}</span>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any(),
                            Err(e) => view! {
                                <ErrorDisplay error=e.to_string() />
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
