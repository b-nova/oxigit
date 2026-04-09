use leptos::prelude::*;

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::{IconLock, IconRepo};
use crate::components::loading::LoadingCard;

use super::RepoInfo;

#[server]
async fn list_repos() -> Result<(String, Vec<RepoInfo>), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    #[cfg(feature = "saas")]
    use crate::server_fns::is_multi_tenant;
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let repos = {
        #[cfg(feature = "saas")]
        {
            if is_multi_tenant().await? {
                // In multi-tenant mode, list from the global index
                let entries = db::list_user_repos_from_index(&pool, user.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
                // Convert RepositoryIndexEntry to Repository-compatible for the UI
                entries.into_iter().map(|e| oxigit_core::models::Repository {
                    id: e.id,
                    owner_id: e.owner_id,
                    name: e.repo_name,
                    description: e.description,
                    is_private: e.is_private,
                    forked_from: None,
                    has_remix: false,
                    created_at: e.created_at,
                    updated_at: e.updated_at,
                }).collect()
            } else {
                db::list_user_repositories(&pool, user.id).await.map_err(|e| ServerFnError::new(e.to_string()))?
            }
        }
        #[cfg(not(feature = "saas"))]
        { db::list_user_repositories(&pool, user.id).await.map_err(|e| ServerFnError::new(e.to_string()))? }
    };

    let username = user.username.clone();
    Ok((username, repos
        .into_iter()
        .map(|r| RepoInfo {
            id: r.id,
            name: r.name,
            description: r.description,
            is_private: r.is_private,
            created_at: r.created_at,
        })
        .collect()))
}

#[component]
pub fn RepoListPage() -> impl IntoView {
    let repos = Resource::new(|| (), |_| list_repos());

    view! {
        <div class="page-header">
            <h1 class="page-title">"Your repositories"</h1>
            <a href="/repos/new" class="btn btn-primary">"New repository"</a>
        </div>
        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
                {move || Suspend::new(async move {
                    match repos.await {
                        Ok((_, repos)) if repos.is_empty() => view! {
                            <div class="empty-state">
                                <div class="empty-state-icon"><IconRepo /></div>
                                <p class="empty-state-title">"No repositories yet."</p>
                                <div class="empty-state-actions">
                                    <a href="/repos/new" class="btn btn-primary">"Create repository"</a>
                                </div>
                            </div>
                        }.into_any(),
                        Ok((username, repos)) => view! {
                            <ul class="list">
                                {repos.into_iter().map(|repo| {
                                    let name = repo.name.clone();
                                    let href = format!("/{}/{}", username, name);
                                    let desc = repo.description.clone();
                                    let created = repo.created_at.clone();
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="flex-row gap-2">
                                                    <a href={href} class="list-item-title">{name}</a>
                                                    {repo.is_private.then(|| view! {
                                                        <span class="badge badge-private">
                                                            <IconLock />
                                                            "Private"
                                                        </span>
                                                    })}
                                                </div>
                                                {(!desc.is_empty()).then(|| view! {
                                                    <p class="list-item-desc">{desc.clone()}</p>
                                                })}
                                            </div>
                                            <span class="list-item-meta">{created}</span>
                                        </li>
                                    }
                                }).collect::<Vec<_>>()}
                            </ul>
                        }.into_any(),
                        Err(e) => view! {
                            <ErrorDisplay error=e.to_string() />
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
