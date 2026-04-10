use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;

#[server]
async fn create_issue(
    owner: String,
    repo: String,
    title: String,
    description: String,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{require_auth, get_repo_pools, sfn_err};
    use oxigit_core::db;

    let user = require_auth().await?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let issue = db::create_issue(&pool, repo_db.id, user.id, &title, &description)
        .await
        .map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/issues/{}", owner, repo, issue.number));
    Ok(())
}

#[component]
pub fn IssueNewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let create_action = ServerAction::<CreateIssue>::new();
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
                <a href={move || format!("/{}/{}/issues", owner(), repo())}>"Issues"</a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"New"</span>
            </h1>
        </div>
        <div class="card">
            {move || error().map(|e| view! {
                <ErrorDisplay error=e />
            })}
            <ActionForm action=create_action>
                <input type="hidden" name="owner" value={move || owner()} />
                <input type="hidden" name="repo" value={move || repo()} />
                <div class="form-group">
                    <label for="title">"Title"</label>
                    <input type="text" id="title" name="title" required />
                </div>
                <div class="form-group">
                    <label for="description">"Description (optional)"</label>
                    <textarea
                        id="description"
                        name="description"
                        rows="6"
                        class="form-textarea"
                    ></textarea>
                </div>
                <button type="submit" class="btn btn-primary">"Submit New Issue"</button>
            </ActionForm>
        </div>
    }
}
