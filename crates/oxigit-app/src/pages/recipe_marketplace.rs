use leptos::prelude::*;

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::IconSearch;
use crate::components::loading::LoadingCard;

#[allow(unused_imports)]
use super::{RecipeListItem, RecipeMarketplaceResponse};

#[server]
async fn fetch_recipe_marketplace(
    query: String,
    sort: String,
    page: i64,
) -> Result<RecipeMarketplaceResponse, ServerFnError> {
    use crate::server_fns::get_pool;
    use oxigit_core::db;

    let pool = get_pool().await?;
    let limit = 20i64;
    let offset = page * limit;

    // In multi-tenant mode, recipes are in tenant DBs and not globally indexed yet.
    #[cfg(feature = "saas")]
    if crate::server_fns::is_multi_tenant().await? {
        return Ok(RecipeMarketplaceResponse { recipes: vec![], total: 0, has_more: false });
    }

    let total = db::count_public_recipes(&pool, &query)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let results = db::search_public_recipes(&pool, &query, &sort, limit, offset)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let recipes: Vec<RecipeListItem> = results.into_iter().map(|r| {
        let tags: Vec<String> = r.recipe.tags.as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        RecipeListItem {
            id: r.recipe.id,
            title: r.recipe.title,
            description: r.recipe.description,
            ai_tool: r.recipe.ai_tool,
            ai_model: r.recipe.ai_model,
            tags,
            prompt_count: r.recipe.prompt_count,
            file_count: r.recipe.file_count,
            vibe_score: r.recipe.vibe_score,
            replay_count: r.recipe.replay_count,
            author: r.author_username,
            repo_owner: r.repo_owner,
            repo_name: r.repo_name,
            created_at: r.recipe.created_at,
        }
    }).collect();

    let has_more = (offset + limit) < total;

    Ok(RecipeMarketplaceResponse { recipes, total, has_more })
}

#[component]
pub fn RecipeMarketplacePage() -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (sort, set_sort) = signal("newest".to_string());
    let (page, set_page) = signal(0i64);

    let data = Resource::new(
        move || (query.get(), sort.get(), page.get()),
        move |(q, s, p)| fetch_recipe_marketplace(q, s, p),
    );

    view! {
        <div class="page-header">
            <h1>"Recipe Marketplace"</h1>
        </div>

        <div class="search-wrapper">
            <IconSearch />
            <input
                type="text"
                class="search-input"
                placeholder="Search recipes..."
                on:input=move |ev| {
                    set_query.set(event_target_value(&ev));
                    set_page.set(0);
                }
            />
        </div>

        <div class="marketplace-filters mb-3">
            <button class="btn btn-sm" class:btn-primary=move || sort.get() == "newest"
                on:click=move |_| { set_sort.set("newest".into()); set_page.set(0); }>
                "Newest"
            </button>
            <button class="btn btn-sm" class:btn-primary=move || sort.get() == "top_vibe"
                on:click=move |_| { set_sort.set("top_vibe".into()); set_page.set(0); }>
                "Top Rated"
            </button>
            <button class="btn btn-sm" class:btn-primary=move || sort.get() == "most_replayed"
                on:click=move |_| { set_sort.set("most_replayed".into()); set_page.set(0); }>
                "Most Replayed"
            </button>
        </div>

        <Suspense fallback=|| view! { <LoadingCard /> }>
            {move || {
                let current_page = page.get();
                Suspend::new(async move {
                    match data.await {
                        Ok(resp) if resp.recipes.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">"No recipes found."</p>
                                <p class="empty-state-text">"Share an AI session as a recipe to get started."</p>
                            </div>
                        }.into_any(),
                        Ok(resp) => {
                            let cards: Vec<AnyView> = resp.recipes.into_iter().map(|r| {
                                let href = format!("/recipes/{}", r.id);
                                let tags: Vec<AnyView> = r.tags.iter().map(|t| {
                                    view! { <span class="recipe-tag">{t.clone()}</span> }.into_any()
                                }).collect();
                                let grade = r.vibe_score.map(|s| {
                                    let g = match s as u8 { 80..=100 => 'A', 60..=79 => 'B', 40..=59 => 'C', 20..=39 => 'D', _ => 'F' };
                                    (s, g)
                                });
                                view! {
                                    <a href={href} class="recipe-card">
                                        <div class="recipe-card-header">
                                            <span class="recipe-card-title">{r.title}</span>
                                            <span class="ai-badge">{r.ai_tool}</span>
                                            {grade.map(|(s, g)| {
                                                let cls = format!("vibe-badge vibe-{}", g);
                                                view! { <span class={cls}>{s.to_string()}</span> }
                                            })}
                                        </div>
                                        {(!r.description.is_empty()).then(|| view! {
                                            <p class="recipe-card-desc">{r.description}</p>
                                        })}
                                        <div class="recipe-card-meta">
                                            <span class="text-tertiary">"by " {r.author}</span>
                                            <span class="text-tertiary">{r.prompt_count} " prompts"</span>
                                            <span class="text-tertiary">{r.file_count} " files"</span>
                                            <span class="text-tertiary">{r.replay_count} " replays"</span>
                                        </div>
                                        {(!tags.is_empty()).then(|| view! {
                                            <div class="recipe-card-tags">{tags}</div>
                                        })}
                                    </a>
                                }.into_any()
                            }).collect();

                            view! {
                                <p class="text-tertiary mb-3">{resp.total} " recipe(s)"</p>
                                <div class="marketplace-grid">{cards}</div>
                                <div class="pagination">
                                    {(current_page > 0).then(|| view! {
                                        <button class="btn btn-sm" on:click=move |_| set_page.set(current_page - 1)>"Previous"</button>
                                    })}
                                    {resp.has_more.then(|| view! {
                                        <button class="btn btn-sm" on:click=move |_| set_page.set(current_page + 1)>"Next"</button>
                                    })}
                                </div>
                            }.into_any()
                        }
                        Err(e) => view! {
                            <ErrorDisplay error=e.to_string() />
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
