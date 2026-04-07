use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[allow(unused_imports)]
use super::{RecipeDetailResponse, RecipeFileInfo, RecipeStepInfo, ReplayTargetRepo};

#[server]
async fn fetch_recipe_detail(recipe_id: i64) -> Result<RecipeDetailResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;

    let recipe = db::get_recipe_by_id(&pool, recipe_id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Recipe not found"))?;

    // Get author and repo info
    let author_user = db::get_user_by_id(&pool, recipe.author_id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let repo_db = sqlx::query_as::<_, oxigit_core::models::Repository>(
        "SELECT * FROM repositories WHERE id = ?",
    )
    .bind(recipe.repo_id)
    .fetch_one(&pool)
    .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let owner_user = db::get_user_by_id(&pool, repo_db.owner_id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let db_steps = db::get_recipe_steps(&pool, recipe_id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let steps: Vec<RecipeStepInfo> = db_steps.into_iter().map(|s| {
        let files: Vec<RecipeFileInfo> = s.files_json.as_ref()
            .and_then(|j| serde_json::from_str::<Vec<serde_json::Value>>(j).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| {
                Some(RecipeFileInfo {
                    path: v.get("path")?.as_str()?.to_string(),
                    content: v.get("content")?.as_str()?.to_string(),
                })
            })
            .collect();

        let diff_html = s.diff_text.as_ref()
            .map(|d| crate::pages::ai_session_detail::render_diff_public(d))
            .unwrap_or_default();

        RecipeStepInfo {
            step_order: s.step_order,
            prompt_text: s.prompt_text,
            commit_message: s.commit_message,
            files,
            diff_html,
        }
    }).collect();

    let tags: Vec<String> = recipe.tags.as_ref()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or_default();

    // Get user's repos for replay target picker
    let (can_replay, user_repos) = if let Some(ref user) = current_user {
        let repos = db::list_user_repositories(&pool, user.id)
            .await.unwrap_or_default();
        let targets: Vec<ReplayTargetRepo> = repos.into_iter().map(|r| {
            let rp = git::repo_path(&data_dir, &user.username, &r.name);
            let branches = git::list_branches(&rp).unwrap_or_default();
            ReplayTargetRepo {
                owner: user.username.clone(),
                name: r.name,
                branches,
            }
        }).collect();
        (true, targets)
    } else {
        (false, vec![])
    };

    Ok(RecipeDetailResponse {
        id: recipe.id,
        title: recipe.title,
        description: recipe.description,
        ai_tool: recipe.ai_tool,
        ai_model: recipe.ai_model,
        tags,
        vibe_score: recipe.vibe_score,
        replay_count: recipe.replay_count,
        author: author_user.username,
        repo_owner: owner_user.username.clone(),
        repo_name: repo_db.name,
        steps,
        can_replay,
        user_repos,
        created_at: recipe.created_at,
    })
}

#[server]
async fn replay_recipe(
    recipe_id: i64,
    target_owner: String,
    target_repo: String,
    target_branch: String,
    mode: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool, get_user_entitlements};
    use oxigit_core::{db, git};

    let user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.team_features {
        return Err(ServerFnError::new(
            "Recipe replay requires a Team plan. Upgrade at /pricing",
        ));
    }

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    let (_, target_repo_db) = db::get_repository(&pool, &target_owner, &target_repo)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let can_push = db::can_push_repo(&pool, &target_repo_db, user.id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?;
    if !can_push {
        return Err(ServerFnError::new("No push access to target repo"));
    }

    let _recipe = db::get_recipe_by_id(&pool, recipe_id)
        .await.map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Recipe not found"))?;

    if mode == "apply" {
        let steps = db::get_recipe_steps(&pool, recipe_id)
            .await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let repo_path = git::repo_path(&data_dir, &target_owner, &target_repo);
        let mut steps_applied = 0i64;

        for step in &steps {
            let files: Vec<(String, String)> = step.files_json.as_ref()
                .and_then(|j| serde_json::from_str::<Vec<serde_json::Value>>(j).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    let path = v.get("path")?.as_str()?.to_string();
                    let content = v.get("content")?.as_str()?.to_string();
                    Some((path, content))
                })
                .collect();

            if files.is_empty() { continue; }

            let file_refs: Vec<(&str, &str, bool)> = files.iter()
                .map(|(p, c)| (p.as_str(), c.as_str(), false))
                .collect();

            let message = format!("recipe: {}", step.commit_message);
            match git::add_files_to_branch(
                &repo_path, &target_branch, &file_refs,
                &message, &user.username, &format!("{}@oxigit", user.username),
            ) {
                Ok(_) => steps_applied += 1,
                Err(e) => {
                    let _ = db::create_recipe_replay(
                        &pool, recipe_id, user.id, target_repo_db.id,
                        &target_branch, "apply", "partial", steps_applied,
                        Some(&e.to_string()),
                    ).await;
                    let _ = db::increment_replay_count(&pool, recipe_id).await;
                    return Err(ServerFnError::new(format!("Replay failed at step {}: {}", steps_applied + 1, e)));
                }
            }
        }

        let _ = db::create_recipe_replay(
            &pool, recipe_id, user.id, target_repo_db.id,
            &target_branch, "apply", "success", steps_applied, None,
        ).await;
    } else {
        // Prompt-only mode — just track the replay
        let _ = db::create_recipe_replay(
            &pool, recipe_id, user.id, target_repo_db.id,
            &target_branch, "prompt_only", "success", 0, None,
        ).await;
    }

    let _ = db::increment_replay_count(&pool, recipe_id).await;
    leptos_axum::redirect(&format!("/{}/{}", target_owner, target_repo));
    Ok(())
}

#[component]
pub fn RecipeDetailPage() -> impl IntoView {
    let params = use_params_map();
    let recipe_id = move || params.read().get("recipe_id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);

    let data = Resource::new(
        move || recipe_id(),
        move |id| fetch_recipe_detail(id),
    );

    let replay_action = ServerAction::<ReplayRecipe>::new();

    view! {
        <Suspense fallback=|| view! { <p class="text-secondary mt-8">"Loading..."</p> }>
            {move || {
                Suspend::new(async move {
                    match data.await {
                        Ok(d) => {
                            let tags: Vec<AnyView> = d.tags.iter().map(|t| {
                                view! { <span class="recipe-tag">{t.clone()}</span> }.into_any()
                            }).collect();

                            let steps: Vec<AnyView> = d.steps.iter().enumerate().map(|(i, step)| {
                                let diff = step.diff_html.clone();
                                let file_count = step.files.len();
                                view! {
                                    <div class="recipe-step">
                                        <div class="recipe-step-header">
                                            <span class="recipe-step-number">{(i + 1).to_string()}</span>
                                            <div class="turn-prompt">
                                                {step.prompt_text.clone().unwrap_or_else(|| step.commit_message.clone())}
                                            </div>
                                        </div>
                                        <details class="turn-diff-toggle">
                                            <summary>{format!("{} file(s)", file_count)}</summary>
                                            <div class="diff-container" inner_html={diff}></div>
                                        </details>
                                    </div>
                                }.into_any()
                            }).collect();

                            // Replay panel
                            let replay_view = if d.can_replay && !d.user_repos.is_empty() {
                                let repo_options: Vec<AnyView> = d.user_repos.iter().map(|r| {
                                    let val = format!("{}/{}", r.owner, r.name);
                                    let label = format!("{}/{}", r.owner, r.name);
                                    view! { <option value={val}>{label}</option> }.into_any()
                                }).collect();

                                Some(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Replay Recipe"</div>
                                        <div class="session-ops">
                                            <ActionForm action=replay_action>
                                                <input type="hidden" name="recipe_id" value={d.id.to_string()} />
                                                <input type="hidden" name="mode" value="apply" />
                                                <div class="session-op-row">
                                                    <select name="target_owner" class="form-input form-input-sm" style="display:none;">
                                                        // Will be set by JS from the combined field
                                                    </select>
                                                    <label>"Target:"</label>
                                                    <select name="target_repo" class="form-input form-input-sm">
                                                        {repo_options}
                                                    </select>
                                                    <input type="text" name="target_branch" value="main" class="form-input form-input-sm" style="width: 120px;" placeholder="Branch" />
                                                    <button type="submit" class="btn btn-primary btn-sm">"Apply Changes"</button>
                                                </div>
                                                <span class="session-op-desc">
                                                    "Creates commits with the recipe's file changes on the target branch."
                                                </span>
                                            </ActionForm>
                                        </div>
                                    </div>
                                })
                            } else { None };

                            let grade = d.vibe_score.map(|s| {
                                let g = match s as u8 { 80..=100 => 'A', 60..=79 => 'B', 40..=59 => 'C', 20..=39 => 'D', _ => 'F' };
                                (s, g)
                            });

                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href="/recipes">"Recipes"</a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span>{d.title.clone()}</span>
                                    </h1>
                                </div>

                                <div class="card mb-4">
                                    <div style="padding: var(--space-4);">
                                        <h2 style="margin: 0 0 var(--space-2);">{d.title}</h2>
                                        {(!d.description.is_empty()).then(|| view! {
                                            <p class="text-secondary mb-3">{d.description.clone()}</p>
                                        })}
                                        <div class="prompt-card-meta">
                                            <span class="ai-badge">{d.ai_tool}</span>
                                            {d.ai_model.map(|m| view! { <span class="tag">{m}</span> })}
                                            {grade.map(|(s, g)| {
                                                let cls = format!("vibe-badge vibe-{}", g);
                                                view! { <span class={cls}>{s.to_string()} " " {g.to_string()}</span> }
                                            })}
                                            <span class="text-tertiary">{d.replay_count} " replays"</span>
                                            <span class="text-tertiary">"by " <a href={format!("/{}", d.author)}>{d.author.clone()}</a></span>
                                            <span class="text-tertiary">
                                                "from " <a href={format!("/{}/{}", d.repo_owner, d.repo_name)}>
                                                    {d.repo_owner.clone()} "/" {d.repo_name.clone()}
                                                </a>
                                            </span>
                                        </div>
                                        {(!tags.is_empty()).then(|| view! {
                                            <div class="mt-2" style="display: flex; gap: var(--space-1); flex-wrap: wrap;">{tags}</div>
                                        })}
                                    </div>
                                </div>

                                <div class="card mb-4">
                                    <div class="card-header">{d.steps.len()} " Step(s)"</div>
                                    <div style="padding: var(--space-4);">{steps}</div>
                                </div>

                                {replay_view}
                            }.into_any()
                        }
                        Err(e) => view! {
                            <div class="flash flash-error">{e.to_string()}</div>
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}
