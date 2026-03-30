use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::icons::IconLock;

use super::RepoInfo;

#[server]
async fn fetch_user_profile(
    username: String,
) -> Result<(String, Vec<RepoInfo>), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let pool = get_pool().await?;
    let current_user = extract_session_user().await;

    let user = db::get_user_by_username(&pool, &username)
        .await
        .map_err(|_| ServerFnError::new("User not found"))?;

    let repos = db::list_user_repositories(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let is_own_profile = current_user.as_ref().map(|u| u.id) == Some(user.id);

    let visible_repos: Vec<RepoInfo> = repos
        .into_iter()
        .filter(|r| !r.is_private || is_own_profile)
        .map(|r| RepoInfo {
            id: r.id,
            name: r.name,
            description: r.description,
            is_private: r.is_private,
            created_at: r.created_at,
        })
        .collect();

    Ok((user.username, visible_repos))
}

#[component]
pub fn UserProfilePage() -> impl IntoView {
    let params = use_params_map();
    let user = move || params.read().get("user").unwrap_or_default();

    let profile = Resource::new(
        move || user(),
        move |username| fetch_user_profile(username),
    );

    view! {
        <Suspense fallback=|| view! { <p class="text-secondary mt-8">"Loading..."</p> }>
            {move || {
                let _username = user();
                Suspend::new(async move {
                    match profile.await {
                        Ok((name, repos)) => view! {
                            <div class="page-header">
                                <h1 class="page-title">{name.clone()}</h1>
                            </div>
                            <div class="card mb-4">
                                <span class="text-secondary">{repos.len()} " repositories"</span>
                            </div>
                            {if repos.is_empty() {
                                view! {
                                    <div class="card">
                                        <div class="empty-state">
                                            <p class="empty-state-title">"No public repositories."</p>
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="card-flush">
                                        <ul class="list">
                                            {repos.into_iter().map(|repo| {
                                                let repo_name = repo.name.clone();
                                                let href = format!("/{}/{}", name, repo_name);
                                                let desc = repo.description.clone();
                                                let created = repo.created_at.clone();
                                                view! {
                                                    <li class="list-item">
                                                        <div>
                                                            <div class="flex-row gap-2">
                                                                <a href={href} class="list-item-title">{repo_name}</a>
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
                                    </div>
                                }.into_any()
                            }}
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
