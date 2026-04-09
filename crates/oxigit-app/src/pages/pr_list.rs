use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::IconPullRequest;
use crate::components::loading::LoadingCard;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrSummary {
    pub number: i64,
    pub title: String,
    pub author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub status: String,
    pub created_at: String,
}

#[server]
pub async fn list_prs(
    owner: String,
    repo: String,
    status: String,
) -> Result<Vec<PrSummary>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_repo_pool};
    use oxigit_core::db;

    let pool = get_repo_pool(&owner, &repo).await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let filter = if status.is_empty() { None } else { Some(status.as_str()) };
    let prs = db::list_pull_requests(&pool, repo_db.id, filter)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(prs
        .into_iter()
        .map(|(pr, author)| PrSummary {
            number: pr.number,
            title: pr.title,
            author: author.username,
            source_branch: pr.source_branch,
            target_branch: pr.target_branch,
            status: pr.status,
            created_at: pr.created_at,
        })
        .collect())
}

#[component]
pub fn PrListPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let (status_filter, set_status_filter) = signal("open".to_string());

    let prs = Resource::new(
        move || (owner(), repo(), status_filter.get()),
        move |(owner, repo, status)| list_prs(owner, repo, status),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Pull Requests"</span>
            </h1>
            <a href={move || format!("/{}/{}/pulls/new", owner(), repo())} class="btn btn-primary">"New Pull Request"</a>
        </div>

        <div class="filter-tabs">
            <button
                class=move || if status_filter.get() == "open" { "filter-tab filter-tab-active" } else { "filter-tab" }
                on:click=move |_| set_status_filter.set("open".to_string())
            >"Open"</button>
            <button
                class=move || if status_filter.get() == "merged" { "filter-tab filter-tab-active" } else { "filter-tab" }
                on:click=move |_| set_status_filter.set("merged".to_string())
            >"Merged"</button>
            <button
                class=move || if status_filter.get() == "closed" { "filter-tab filter-tab-active" } else { "filter-tab" }
                on:click=move |_| set_status_filter.set("closed".to_string())
            >"Closed"</button>
        </div>

        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
                {move || {
                    let owner_name = owner();
                    let repo_name = repo();
                    let current_status = status_filter.get();
                    Suspend::new(async move {
                        match prs.await {
                            Ok(prs) if prs.is_empty() => {
                                let msg = format!("No {} pull requests.", current_status);
                                view! {
                                    <div class="empty-state">
                                        <div class="empty-state-icon"><IconPullRequest /></div>
                                        <p class="empty-state-title">{msg}</p>
                                    </div>
                                }.into_any()
                            },
                            Ok(prs) => view! {
                                <ul class="list">
                                    {prs.into_iter().map(|pr| {
                                        let href = format!("/{}/{}/pulls/{}", owner_name, repo_name, pr.number);
                                        let title = pr.title.clone();
                                        let number = pr.number;
                                        let author = pr.author.clone();
                                        let created = pr.created_at.clone();
                                        let badge_class = match pr.status.as_str() {
                                            "open" => "badge badge-open",
                                            "merged" => "badge badge-merged",
                                            "closed" => "badge badge-closed",
                                            _ => "badge",
                                        };
                                        let target = pr.target_branch.clone();
                                        let source = pr.source_branch.clone();
                                        view! {
                                            <li class="list-item">
                                                <div class="flex-1">
                                                    <div class="flex-row gap-2">
                                                        <a href={href} class="font-semibold">{title}</a>
                                                        <span class="text-tertiary">{"#"}{number}</span>
                                                    </div>
                                                    <div class="list-item-meta mt-1 flex-row gap-2" style="flex-wrap: wrap;">
                                                        <span class={badge_class}>{pr.status.clone()}</span>
                                                        <span>{author}</span>
                                                        <span>
                                                            <span class="badge-branch">{target}</span>
                                                            " \u{2190} "
                                                            <span class="badge-branch">{source}</span>
                                                        </span>
                                                        <span>{created}</span>
                                                    </div>
                                                </div>
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
