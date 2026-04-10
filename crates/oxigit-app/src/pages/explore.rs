use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::IconSearch;
use crate::components::loading::LoadingCard;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExploreRepo {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub has_remix: bool,
}

#[server]
async fn explore_repos(query: String, remixable_only: bool) -> Result<Vec<ExploreRepo>, ServerFnError> {
    use crate::server_fns::{get_pool, sfn_err};
    #[cfg(feature = "saas")]
    use crate::server_fns::is_multi_tenant;
    use oxigit_core::db;

    let pool = get_pool().await?;
    let results = {
        #[cfg(feature = "saas")]
        {
            if is_multi_tenant().await? {
                // In multi-tenant mode, use the repository_index in the control DB.
                // The index doesn't track has_remix, so remixable filtering is not
                // available in multi-tenant mode yet.
                db::search_public_repos_from_index(&pool, &query)
                    .await
                    .map_err(sfn_err)?
                    .into_iter()
                    .map(|e| ExploreRepo {
                        owner: e.owner_username,
                        name: e.repo_name,
                        description: e.description,
                        created_at: e.created_at,
                        has_remix: false,
                    })
                    .collect()
            } else {
                let results = if remixable_only {
                    db::search_remixable_repositories(&pool, &query)
                        .await
                        .map_err(sfn_err)?
                } else {
                    db::search_public_repositories(&pool, &query)
                        .await
                        .map_err(sfn_err)?
                };
                results
                    .into_iter()
                    .map(|(user, repo)| ExploreRepo {
                        owner: user.username,
                        name: repo.name,
                        description: repo.description,
                        created_at: repo.created_at,
                        has_remix: repo.has_remix,
                    })
                    .collect()
            }
        }
        #[cfg(not(feature = "saas"))]
        {
            let results = if remixable_only {
                db::search_remixable_repositories(&pool, &query)
                    .await
                    .map_err(sfn_err)?
            } else {
                db::search_public_repositories(&pool, &query)
                    .await
                    .map_err(sfn_err)?
            };
            results
                .into_iter()
                .map(|(user, repo)| ExploreRepo {
                    owner: user.username,
                    name: repo.name,
                    description: repo.description,
                    created_at: repo.created_at,
                    has_remix: repo.has_remix,
                })
                .collect()
        }
    };

    Ok(results)
}

#[component]
pub fn ExplorePage() -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (remixable_only, set_remixable_only) = signal(false);

    let repos = Resource::new(
        move || (query.get(), remixable_only.get()),
        move |(q, remix)| explore_repos(q, remix),
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

        <div class="explore-filters mb-4">
            <button
                class=move || if !remixable_only.get() { "btn btn-sm btn-active" } else { "btn btn-sm" }
                on:click=move |_| set_remixable_only.set(false)
            >"All"</button>
            <button
                class=move || if remixable_only.get() { "btn btn-sm btn-ai btn-active" } else { "btn btn-sm btn-ai" }
                on:click=move |_| set_remixable_only.set(true)
            >"Remixable"</button>
        </div>

        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
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
                                    let is_remix = repo.has_remix;
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="list-item-title">
                                                    <a href={href}>{full_name}</a>
                                                    {is_remix.then(|| view! {
                                                        <span class="remix-card-badge" style="margin-left: var(--space-2);">"Remixable"</span>
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
