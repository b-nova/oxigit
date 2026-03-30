use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueSummary {
    pub number: i64,
    pub title: String,
    pub author: String,
    pub status: String,
    pub created_at: String,
}

#[server]
async fn list_issues(
    owner: String,
    repo: String,
    status: String,
) -> Result<Vec<IssueSummary>, ServerFnError> {
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

    let filter = if status.is_empty() { None } else { Some(status.as_str()) };
    let issues = db::list_issues(&pool, repo_db.id, filter)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(issues
        .into_iter()
        .map(|(issue, author)| IssueSummary {
            number: issue.number,
            title: issue.title,
            author: author.username,
            status: issue.status,
            created_at: issue.created_at,
        })
        .collect())
}

#[component]
pub fn IssueListPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let (status_filter, set_status_filter) = signal("open".to_string());

    let issues = Resource::new(
        move || (owner(), repo(), status_filter.get()),
        move |(owner, repo, status)| list_issues(owner, repo, status),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Issues"</span>
            </h1>
            <a href={move || format!("/{}/{}/issues/new", owner(), repo())} class="btn btn-primary">"New Issue"</a>
        </div>

        <div class="filter-tabs">
            <button
                class=move || if status_filter.get() == "open" { "filter-tab filter-tab-active" } else { "filter-tab" }
                on:click=move |_| set_status_filter.set("open".to_string())
            >"Open"</button>
            <button
                class=move || if status_filter.get() == "closed" { "filter-tab filter-tab-active" } else { "filter-tab" }
                on:click=move |_| set_status_filter.set("closed".to_string())
            >"Closed"</button>
        </div>

        <div class="card-flush">
            <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
                {move || {
                    let owner_name = owner();
                    let repo_name = repo();
                    let current_status = status_filter.get();
                    Suspend::new(async move {
                        match issues.await {
                            Ok(issues) if issues.is_empty() => {
                                let msg = format!("No {} issues.", current_status);
                                view! {
                                    <div class="empty-state">{msg}</div>
                                }.into_any()
                            },
                            Ok(issues) => view! {
                                <ul class="list">
                                    {issues.into_iter().map(|issue| {
                                        let href = format!("/{}/{}/issues/{}", owner_name, repo_name, issue.number);
                                        let title = issue.title.clone();
                                        let number = issue.number;
                                        let author = issue.author.clone();
                                        let created = issue.created_at.clone();
                                        let badge_class = if issue.status == "open" { "badge badge-open" } else { "badge badge-closed" };
                                        view! {
                                            <li class="list-item">
                                                <div class="flex-1">
                                                    <div class="flex-row gap-2">
                                                        <a href={href} class="font-semibold">{title}</a>
                                                        <span class="text-tertiary">{"#"}{number}</span>
                                                    </div>
                                                    <div class="list-item-meta mt-1">
                                                        <span class={badge_class}>{issue.status.clone()}</span>
                                                        " opened by " {author} " on " {created}
                                                    </div>
                                                </div>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any(),
                            Err(e) => view! {
                                <div class="flash flash-error" style="margin: var(--space-4);">{e.to_string()}</div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
