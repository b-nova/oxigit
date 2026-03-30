use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::icons::IconSearch;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExploreRepo {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[server]
async fn explore_repos(query: String) -> Result<Vec<ExploreRepo>, ServerFnError> {
    use crate::server_fns::get_pool;
    use oxigit_core::db;

    let pool = get_pool().await?;
    let results = db::search_public_repositories(&pool, &query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(results
        .into_iter()
        .map(|(user, repo)| ExploreRepo {
            owner: user.username,
            name: repo.name,
            description: repo.description,
            created_at: repo.created_at,
        })
        .collect())
}

#[component]
pub fn ExplorePage() -> impl IntoView {
    let (query, set_query) = signal(String::new());

    let repos = Resource::new(
        move || query.get(),
        move |q| explore_repos(q),
    );

    view! {
        <div class="page-header">
            <h1 class="page-title">"Explore"</h1>
        </div>

        <div class="search-wrapper">
            <IconSearch />
            <input
                type="text"
                class="search-input"
                placeholder="Search repositories..."
                on:input=move |ev| {
                    set_query.set(event_target_value(&ev));
                }
            />
        </div>

        <div class="card-flush">
            <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
                {move || Suspend::new(async move {
                    match repos.await {
                        Ok(repos) if repos.is_empty() => view! {
                            <div class="empty-state">
                                <p class="empty-state-title">"No repositories found."</p>
                            </div>
                        }.into_any(),
                        Ok(repos) => view! {
                            <ul class="list">
                                {repos.into_iter().map(|repo| {
                                    let href = format!("/{}/{}", repo.owner, repo.name);
                                    let full_name = format!("{}/{}", repo.owner, repo.name);
                                    let desc = repo.description.clone();
                                    let created = repo.created_at.clone();
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="list-item-title">
                                                    <a href={href}>{full_name}</a>
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
                            <div class="flash flash-error" style="margin: var(--space-4);">{e.to_string()}</div>
                        }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
