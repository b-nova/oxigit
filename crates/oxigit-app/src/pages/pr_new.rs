use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;

#[server]
async fn get_branches(owner: String, repo: String) -> Result<Vec<String>, ServerFnError> {
    use crate::server_fns::{get_repo_path, get_repo_pools, sfn_err};
    use oxigit_core::{db, git};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let repo_path = get_repo_path(&owner, &repo).await?;
    Ok(git::list_branches(&repo_path).unwrap_or_default())
}

#[server]
async fn create_pr(
    owner: String,
    repo: String,
    title: String,
    description: String,
    source_branch: String,
    target_branch: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_repo_path, get_repo_pools, sfn_err};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    // Verify branches exist
    let repo_path = get_repo_path(&owner, &repo).await?;
    if !git::branch_exists(&repo_path, &source_branch) {
        return Err(ServerFnError::new(format!(
            "Branch '{}' not found",
            source_branch
        )));
    }
    if !git::branch_exists(&repo_path, &target_branch) {
        return Err(ServerFnError::new(format!(
            "Branch '{}' not found",
            target_branch
        )));
    }

    let pr = db::create_pull_request(
        &pool,
        repo_db.id,
        user.id,
        &title,
        &description,
        &source_branch,
        &target_branch,
    )
    .await
    .map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/pulls/{}", owner, repo, pr.number));
    Ok(())
}

#[component]
pub fn PrNewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let branches = Resource::new(move || (owner(), repo()), move |(o, r)| get_branches(o, r));

    let create_action = ServerAction::<CreatePr>::new();
    let error = move || {
        create_action
            .value()
            .get()
            .and_then(|r| r.err().map(|e| e.to_string()))
    };

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"New Pull Request"</span>
            </h1>
        </div>
        <div class="card">
            {move || error().map(|e| view! {
                <ErrorDisplay error=e />
            })}
            <ActionForm action=create_action>
                <input type="hidden" name="owner" value={move || owner()} />
                <input type="hidden" name="repo" value={move || repo()} />

                <div class="flex-row gap-4 mb-4">
                    <div class="form-group flex-1">
                        <label for="target_branch">"Base branch"</label>
                        <Suspense fallback=|| view! { <select id="target_branch" name="target_branch" class="form-select"><option>"Loading..."</option></select> }>
                            {move || Suspend::new(async move {
                                match branches.await {
                                    Ok(bs) => view! {
                                        <select id="target_branch" name="target_branch" class="form-select">
                                            {bs.iter().map(|b| {
                                                let name = b.clone();
                                                { let n = name.clone(); view! { <option value={name}>{n}</option> } }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                    }.into_any(),
                                    Err(_) => view! { <select class="form-select"><option>"Error loading branches"</option></select> }.into_any(),
                                }
                            })}
                        </Suspense>
                    </div>
                    <div class="items-end text-secondary" style="padding-bottom: 0.5rem;">
                        "\u{2190}"
                    </div>
                    <div class="form-group flex-1">
                        <label for="source_branch">"Compare branch"</label>
                        <Suspense fallback=|| view! { <select id="source_branch" name="source_branch" class="form-select"><option>"Loading..."</option></select> }>
                            {move || Suspend::new(async move {
                                match branches.await {
                                    Ok(bs) => view! {
                                        <select id="source_branch" name="source_branch" class="form-select">
                                            {bs.iter().rev().map(|b| {
                                                let name = b.clone();
                                                { let n = name.clone(); view! { <option value={name}>{n}</option> } }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                    }.into_any(),
                                    Err(_) => view! { <select class="form-select"><option>"Error"</option></select> }.into_any(),
                                }
                            })}
                        </Suspense>
                    </div>
                </div>

                <div class="form-group">
                    <label for="title">"Title"</label>
                    <input type="text" id="title" name="title" required />
                </div>
                <div class="form-group">
                    <label for="description">"Description (optional)"</label>
                    <textarea
                        id="description"
                        name="description"
                        rows="4"
                        class="form-textarea"
                    ></textarea>
                </div>
                <button type="submit" class="btn btn-primary">"Create Pull Request"</button>
            </ActionForm>
        </div>
    }
}
