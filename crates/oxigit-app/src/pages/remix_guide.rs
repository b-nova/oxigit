use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemixGuideData {
    pub source_name: Option<String>,
    pub remix_html: String,
    pub clone_url: String,
}

#[server]
async fn fetch_remix_guide(
    owner: String,
    repo: String,
) -> Result<RemixGuideData, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_base_url, get_repo_path, get_repo_pools};
    use oxigit_core::{db, git};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user().await;
    let base_url = get_base_url().await;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;
    let default_ref = git::default_branch(&repo_path)
        .unwrap_or(None)
        .unwrap_or_else(|| "main".to_string());

    let remix_html = if let Ok(content) = git::read_blob(&repo_path, &default_ref, "REMIX.md") {
        if let Ok(text) = String::from_utf8(content) {
            use pulldown_cmark::{Parser, html::push_html};
            let parser = Parser::new(&text);
            let mut output = String::new();
            push_html(&mut output, parser);
            output
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let source_name = db::get_fork_source_cross(&control_pool, &pool, &repo_db)
        .await
        .ok()
        .flatten()
        .map(|(user, repo)| format!("{}/{}", user.username, repo.name));

    let clone_url = format!("{}/{}/{}.git", base_url, owner, repo);

    Ok(RemixGuideData {
        source_name,
        remix_html,
        clone_url,
    })
}

#[component]
pub fn RemixGuidePage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let guide = Resource::new(
        move || (owner(), repo()),
        move |(owner, repo)| fetch_remix_guide(owner, repo),
    );

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match guide.await {
                        Ok(data) => {
                            view! {
                                <div class="remix-guide">
                                    <div class="remix-guide-header">
                                        <h1 class="remix-guide-title">"You remixed a project!"</h1>
                                        {data.source_name.map(|source| view! {
                                            <p class="text-secondary">
                                                "Based on " <a href={format!("/{}", source)}>{source.clone()}</a>
                                            </p>
                                        })}
                                    </div>

                                    <div class="card mb-4">
                                        <div class="card-header">"Get started"</div>
                                        <div style="padding: var(--space-4);">
                                            <p class="mb-2">"Clone your remix:"</p>
                                            <code class="clone-bar-url" style="display: block; margin-bottom: var(--space-3);">
                                                {data.clone_url}
                                            </code>
                                            <a href={format!("/{}/{}", owner_name, repo_name)} class="btn btn-primary">
                                                "Go to your repo"
                                            </a>
                                        </div>
                                    </div>

                                    {(!data.remix_html.is_empty()).then(|| {
                                        let html = data.remix_html.clone();
                                        view! {
                                            <div class="remix-card">
                                                <div class="remix-card-header">
                                                    <span class="remix-card-title">"Remix Guide"</span>
                                                </div>
                                                <div class="remix-card-content" inner_html={html}></div>
                                            </div>
                                        }
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
