use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_params_map};
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlobResponse {
    pub file_path: String,
    pub file_name: String,
    pub line_count: usize,
    pub highlighted_html: String,
    pub is_binary: bool,
}

#[server]
async fn get_blob(
    owner: String,
    repo: String,
    path: String,
    git_ref: String,
) -> Result<BlobResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_repo_pool};
    use oxigit_core::{db, git};

    let pool = get_repo_pool(&owner, &repo).await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);

    let git_ref = if git_ref.is_empty() {
        git::default_branch(&repo_path)
            .unwrap_or(None)
            .unwrap_or_else(|| "main".to_string())
    } else {
        git_ref
    };

    let content = git::read_blob(&repo_path, &git_ref, &path)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let file_name = path.rsplit('/').next().unwrap_or(&path).to_string();

    let text = match String::from_utf8(content) {
        Ok(t) => t,
        Err(_) => {
            return Ok(BlobResponse {
                file_path: path,
                file_name,
                line_count: 0,
                highlighted_html: "<em>(binary file)</em>".to_string(),
                is_binary: true,
            });
        }
    };

    let line_count = text.lines().count();
    let highlighted_html = highlight_code(&text, &file_name);

    Ok(BlobResponse {
        file_path: path,
        file_name,
        line_count,
        highlighted_html,
        is_binary: false,
    })
}

#[cfg(feature = "ssr")]
fn highlight_code(code: &str, filename: &str) -> String {
    use syntect::highlighting::ThemeSet;
    use syntect::html::highlighted_html_for_string;
    use syntect::parsing::SyntaxSet;

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

    let syntax = ss
        .find_syntax_for_file(filename)
        .ok()
        .flatten()
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    match highlighted_html_for_string(code, &ss, syntax, theme) {
        Ok(html) => html,
        Err(_) => {
            // Fallback: plain text with HTML escaping
            format!(
                "<pre>{}</pre>",
                code.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            )
        }
    }
}

#[component]
pub fn RepoBlobPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner");
    let repo = move || params.read().get("repo");
    let path = move || params.read().get("path");

    let location = use_location();
    let git_ref = move || {
        let search = location.search.get();
        search
            .strip_prefix('?')
            .unwrap_or(&search)
            .split('&')
            .find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k == "ref").then(|| v.to_string())
            })
            .unwrap_or_default()
    };

    let blob = Resource::new(
        move || (owner(), repo(), path(), git_ref()),
        move |(owner, repo, path, git_ref)| {
            get_blob(
                owner.unwrap_or_default(),
                repo.unwrap_or_default(),
                path.unwrap_or_default(),
                git_ref,
            )
        },
    );

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner().unwrap_or_default();
                let repo_name = repo().unwrap_or_default();
                Suspend::new(async move {
                    match blob.await {
                        Ok(resp) => {
                            let blame_href = {
                                let ref_param = if git_ref().is_empty() { String::new() } else { format!("?ref={}", git_ref()) };
                                format!("/{}/{}/blame/{}{}", owner_name, repo_name, resp.file_path, ref_param)
                            };
                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span class="text-secondary">{resp.file_path.clone()}</span>
                                    </h1>
                                    {(!resp.is_binary).then(|| view! {
                                        <a href={blame_href} class="btn btn-sm">"Blame"</a>
                                    })}
                                </div>
                                <div class="card-flush">
                                    <div class="blob-header">
                                        <span class="blob-filename">{resp.file_name.clone()}</span>
                                        <span class="blob-linecount">{resp.line_count} " lines"</span>
                                    </div>
                                    <div class="blob-content" inner_html={resp.highlighted_html}></div>
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
