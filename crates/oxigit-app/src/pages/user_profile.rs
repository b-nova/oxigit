use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::IconLock;
use crate::components::loading::LoadingPage;

use super::RepoInfo;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserProfileInfo {
    pub username: String,
    pub plan: String,
    pub is_founding_member: bool,
    pub founding_slot: Option<i64>,
    pub repos: Vec<RepoInfo>,
}

#[server]
async fn fetch_user_profile(
    username: String,
) -> Result<UserProfileInfo, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, is_multi_tenant};
    use oxigit_core::db;

    let pool = get_control_pool().await?;
    let current_user = extract_session_user().await;

    let user = db::get_user_by_username(&pool, &username)
        .await
        .map_err(|_| ServerFnError::new("User not found"))?;

    let is_own_profile = current_user.as_ref().map(|u| u.id) == Some(user.id);

    let visible_repos: Vec<RepoInfo> = if is_multi_tenant().await? {
        db::list_user_repos_from_index(&pool, user.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .into_iter()
            .filter(|r| !r.is_private || is_own_profile)
            .map(|r| RepoInfo {
                id: r.id,
                name: r.repo_name,
                description: r.description,
                is_private: r.is_private,
                created_at: r.created_at,
            })
            .collect()
    } else {
        db::list_user_repositories(&pool, user.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .into_iter()
            .filter(|r| !r.is_private || is_own_profile)
            .map(|r| RepoInfo {
                id: r.id,
                name: r.name,
                description: r.description,
                is_private: r.is_private,
                created_at: r.created_at,
            })
            .collect()
    };

    let plan = db::get_user_plan(&pool, user.id)
        .await
        .unwrap_or_else(|_| "free".to_string());
    let founding_slot = db::get_founding_member_slot(&pool, user.id)
        .await
        .unwrap_or(None);
    let is_founding = founding_slot.is_some();

    Ok(UserProfileInfo {
        username: user.username,
        plan,
        is_founding_member: is_founding,
        founding_slot,
        repos: visible_repos,
    })
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
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let _username = user();
                Suspend::new(async move {
                    match profile.await {
                        Ok(info) => {
                            let name = info.username.clone();
                            let repos = info.repos;
                            let plan = info.plan;
                            let is_founding = info.is_founding_member;
                            let founding_slot = info.founding_slot;
                            view! {
                            <div class="page-header">
                                <h1 class="page-title">
                                    {name.clone()}
                                    {(plan != "free" && !is_founding).then(|| {
                                        let class = format!("badge badge-{}", plan);
                                        let label = match plan.as_str() {
                                            "flat" => "Flat",
                                            "team" => "Team",
                                            _ => &plan,
                                        };
                                        view! { <span class={class}>{label}</span> }
                                    })}
                                </h1>
                            </div>
                            {is_founding.then(|| {
                                let badge_src = format!("/api/badge/{}.svg", name);
                                let badge_url = format!("https://oxigit.com/api/badge/{}.svg", name);
                                let download_name = format!("oxigit-founding-badge-{}.svg", name);
                                let slot_display = founding_slot.map(|s| format!("#{:03}", s)).unwrap_or_default();
                                let alt = format!("Oxigit Founding Member {}", slot_display);
                                let embed_code = format!("<img src=\"{}\" alt=\"Oxigit Founding Member\" height=\"120\" />", badge_url);
                                view! {
                                    <div class="founding-badge-section card">
                                        <div class="founding-badge-display">
                                            <img
                                                src={badge_src.clone()}
                                                alt={alt}
                                                class="founding-badge-img"
                                            />
                                        </div>
                                        <div class="founding-badge-actions">
                                            <a
                                                href={badge_src}
                                                download={download_name}
                                                class="btn btn-sm btn-outline"
                                            >
                                                "Download Badge"
                                            </a>
                                            <div class="founding-badge-embed">
                                                <span class="text-secondary text-sm">"Embed on your site:"</span>
                                                <code class="founding-badge-code">{embed_code}</code>
                                            </div>
                                        </div>
                                    </div>
                                }
                            })}
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
                        }.into_any()},
                        Err(e) => view! {
                            <ErrorDisplay error=e.to_string() />
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
