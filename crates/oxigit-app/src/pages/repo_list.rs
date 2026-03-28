use leptos::prelude::*;

use super::RepoInfo;

#[server]
async fn list_repos() -> Result<Vec<RepoInfo>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let repos = db::list_user_repositories(&pool, user.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(repos
        .into_iter()
        .map(|r| RepoInfo {
            id: r.id,
            name: r.name,
            description: r.description,
            is_private: r.is_private,
            created_at: r.created_at,
        })
        .collect())
}

#[component]
pub fn RepoListPage() -> impl IntoView {
    let repos = Resource::new(|| (), |_| list_repos());

    view! {
        <div class="page-header">
            <h1>"Your repositories"</h1>
            <a href="/repos/new" class="btn btn-primary">"New repository"</a>
        </div>
        <div class="card">
            <Suspense fallback=|| view! { <p>"Loading..."</p> }>
                {move || Suspend::new(async move {
                    match repos.await {
                        Ok(repos) if repos.is_empty() => view! {
                            <p style="text-align: center; padding: 2rem; color: var(--text-secondary);">
                                "No repositories yet. " <a href="/repos/new">"Create one!"</a>
                            </p>
                        }.into_any(),
                        Ok(repos) => view! {
                            <ul class="repo-list">
                                {repos.into_iter().map(|repo| {
                                    let name = repo.name.clone();
                                    let href = format!("/repos/{}", name);
                                    let desc = repo.description.clone();
                                    let created = repo.created_at.clone();
                                    view! {
                                        <li class="repo-item">
                                            <div>
                                                <div class="repo-name">
                                                    <a href={href}>{name}</a>
                                                    {repo.is_private.then(|| view! {
                                                        <span style="margin-left: 0.5rem; font-size: 0.75rem; color: var(--text-secondary); border: 1px solid var(--border); padding: 0.125rem 0.375rem; border-radius: 1rem;">"Private"</span>
                                                    })}
                                                </div>
                                                {(!desc.is_empty()).then(|| view! {
                                                    <p class="repo-description">{desc.clone()}</p>
                                                })}
                                            </div>
                                            <span class="repo-meta">{created}</span>
                                        </li>
                                    }
                                }).collect::<Vec<_>>()}
                            </ul>
                        }.into_any(),
                        Err(e) => view! {
                            <div class="flash flash-error">{e.to_string()}</div>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
