use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_params_map};

use crate::components::icons::{IconFile, IconFolder, IconLock};

#[allow(unused_imports)]
use super::{CommitSummary, RepoInfo, RepoTreeResponse, TreeEntryInfo};

#[server]
async fn fetch_repo_tree(
    owner: String,
    repo: String,
    git_ref: String,
    path: String,
) -> Result<RepoTreeResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_base_url, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;
    let base_url = get_base_url().await;

    let (_owner_user, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let viewer_id = current_user.as_ref().map(|u| u.id);
    if !db::can_access_repo(&repo_db, viewer_id) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);

    let branches = git::list_branches(&repo_path)
        .unwrap_or_default();

    let effective_ref = if git_ref.is_empty() {
        git::default_branch(&repo_path)
            .unwrap_or(None)
            .unwrap_or_else(|| "main".to_string())
    } else {
        git_ref
    };

    let entries = git::list_tree(&repo_path, &effective_ref, &path)
        .unwrap_or_default()
        .into_iter()
        .map(|e| TreeEntryInfo {
            name: e.name,
            is_dir: e.is_dir,
            size: e.size,
        })
        .collect();

    let commit = git::get_latest_commit(&repo_path, &effective_ref)
        .unwrap_or(None)
        .map(|c| CommitSummary {
            message: c.message,
            author: c.author,
            time: c.time,
        });

    // Fork info (before moving fields out of repo_db)
    let forked_from = match db::get_fork_source(&pool, &repo_db).await.unwrap_or(None) {
        Some((fork_owner, fork_repo)) => Some(format!("{}/{}", fork_owner.username, fork_repo.name)),
        None => None,
    };

    let is_owner = viewer_id == Some(repo_db.owner_id);
    let can_fork = viewer_id.is_some() && !is_owner;

    let info = RepoInfo {
        id: repo_db.id,
        name: repo_db.name,
        description: repo_db.description,
        is_private: repo_db.is_private,
        created_at: repo_db.created_at,
    };

    // Check for README and REMIX.md, render markdown
    let (readme_html, remix_html) = if path.is_empty() {
        let readme_names = ["README.md", "readme.md", "Readme.md"];
        let mut readme = None;
        for name in &readme_names {
            if let Ok(content) = git::read_blob(&repo_path, &effective_ref, name) {
                if let Ok(text) = String::from_utf8(content) {
                    use pulldown_cmark::{Parser, html::push_html};
                    let parser = Parser::new(&text);
                    let mut output = String::new();
                    push_html(&mut output, parser);
                    readme = Some(output);
                    break;
                }
            }
        }
        let remix = if let Ok(content) = git::read_blob(&repo_path, &effective_ref, "REMIX.md") {
            if let Ok(text) = String::from_utf8(content) {
                use pulldown_cmark::{Parser, html::push_html};
                let parser = Parser::new(&text);
                let mut output = String::new();
                push_html(&mut output, parser);
                Some(output)
            } else { None }
        } else { None };
        (readme, remix)
    } else {
        (None, None)
    };

    Ok(RepoTreeResponse {
        info,
        entries,
        commit,
        branches,
        current_ref: effective_ref,
        readme_html,
        remix_html,
        forked_from,
        can_fork,
        is_owner,
        base_url,
    })
}

#[server]
async fn fork_repo(owner: String, repo: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;

    // Check if source repo has REMIX.md before forking
    let source_path = git::repo_path(&data_dir, &owner, &repo);
    let default_ref = git::default_branch(&source_path).unwrap_or(None).unwrap_or_else(|| "main".to_string());
    let has_remix = git::read_blob(&source_path, &default_ref, "REMIX.md").is_ok();

    let forked = db::fork_repository(&pool, &owner, &repo, user.id, &data_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if has_remix {
        leptos_axum::redirect(&format!("/{}/{}/remix-guide", user.username, forked.name));
    } else {
        leptos_axum::redirect(&format!("/{}/{}", user.username, forked.name));
    }
    Ok(())
}

#[component]
pub fn RepoViewPage() -> impl IntoView {
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

    let tree = Resource::new(
        move || (owner(), repo(), path(), git_ref()),
        move |(owner, repo, path, git_ref)| {
            fetch_repo_tree(
                owner.unwrap_or_default(),
                repo.unwrap_or_default(),
                git_ref,
                path.unwrap_or_default(),
            )
        },
    );

    view! {
        <Suspense fallback=|| view! { <p class="text-secondary mt-8">"Loading..."</p> }>
            {move || Suspend::new(async move {
                match tree.await {
                    Ok(resp) => {
                        let owner_name = owner().unwrap_or_default();
                        let repo_name = resp.info.name.clone();
                        let clone_url = format!("{}/{}/{}.git", resp.base_url, owner_name, repo_name);
                        let clone_url_for_instructions = clone_url.clone();

                        view! {
                            <div class="page-header">
                                <div>
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}", owner_name)}>{owner_name.clone()}</a>
                                        <span class="breadcrumb-sep">"/"</span>
                                        <strong>{repo_name.clone()}</strong>
                                        {resp.info.is_private.then(|| view! {
                                            <span class="badge badge-private ml-2">
                                                <IconLock />
                                                "Private"
                                            </span>
                                        })}
                                    </h1>
                                    {resp.forked_from.clone().map(|source| view! {
                                        <p class="text-secondary mt-1" style="font-size: 0.8125rem;">
                                            "Forked from " <a href={format!("/{}", source)}>{source.clone()}</a>
                                        </p>
                                    })}
                                </div>
                                <div class="flex-row gap-2">
                                    {resp.is_owner.then(|| {
                                        let settings_href = format!("/{}/{}/settings", owner_name, repo_name);
                                        view! {
                                            <a href={settings_href} class="btn">"Settings"</a>
                                        }
                                    })}
                                    {resp.can_fork.then(|| {
                                        let fork_owner = owner_name.clone();
                                        let fork_repo = repo_name.clone();
                                        let is_remix = resp.remix_html.is_some();
                                        let fork_action = ServerAction::<ForkRepo>::new();
                                        view! {
                                            <ActionForm action=fork_action>
                                                <input type="hidden" name="owner" value={fork_owner} />
                                                <input type="hidden" name="repo" value={fork_repo} />
                                                <button type="submit" class={if is_remix { "btn btn-primary" } else { "btn" }}>
                                                    {if is_remix { "Remix" } else { "Fork" }}
                                                </button>
                                            </ActionForm>
                                        }
                                    })}
                                </div>
                            </div>

                            // Clone URL
                            <div class="clone-bar">
                                <span class="clone-bar-label">"Clone:"</span>
                                <code class="clone-bar-url">{clone_url}</code>
                            </div>

                            // Branch selector + nav buttons
                            {if !resp.branches.is_empty() {
                                let cr = resp.current_ref.clone();
                                let branches = resp.branches.clone();
                                let nav_owner = owner_name.clone();
                                let nav_repo = repo_name.clone();
                                let issues_href = format!("/{}/{}/issues", owner_name, repo_name);
                                let commits_href = format!("/{}/{}/commits", owner_name, repo_name);
                                let pulls_href = format!("/{}/{}/pulls", owner_name, repo_name);
                                let ai_href = format!("/{}/{}/ai", owner_name, repo_name);
                                Some(view! {
                                    <div class="flex-row gap-2 mb-4">
                                        <select
                                            class="branch-select"
                                            on:change=move |ev| {
                                                let selected = event_target_value(&ev);
                                                let current_path = path().unwrap_or_default();
                                                let url = if current_path.is_empty() {
                                                    format!("/{}/{}?ref={}", nav_owner, nav_repo, selected)
                                                } else {
                                                    format!("/{}/{}/tree/{}?ref={}", nav_owner, nav_repo, current_path, selected)
                                                };
                                                let _ = window().location().set_href(&url);
                                            }
                                        >
                                            {branches.iter().map(|b| {
                                                let is_selected = *b == cr;
                                                view! { <option value={b.clone()} selected=is_selected>{b.clone()}</option> }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                        <a href={issues_href} class="btn btn-sm">"Issues"</a>
                                        <a href={commits_href} class="btn btn-sm">"Commits"</a>
                                        <a href={pulls_href} class="btn btn-sm">"Pull Requests"</a>
                                        <a href={ai_href} class="btn btn-sm btn-ai">"AI"</a>
                                    </div>
                                })
                            } else {
                                None
                            }}

                            // Latest commit
                            {resp.commit.map(|c| {
                                let msg = c.message.clone();
                                let author = c.author.clone();
                                let time = c.time.clone();
                                view! {
                                    <div class="commit-item" style="background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-lg) var(--radius-lg) 0 0; margin-bottom: 0;">
                                        <div>
                                            <span class="font-semibold">{author}</span>
                                            " "
                                            <span>{msg}</span>
                                        </div>
                                        <span class="text-tertiary" style="font-size: 0.8125rem;">{time}</span>
                                    </div>
                                }
                            })}

                            // Path breadcrumb
                            {
                                let current_path = path().unwrap_or_default();
                                if current_path.is_empty() {
                                    None
                                } else {
                                    let bc_owner = owner_name.clone();
                                    let bc_repo = repo_name.clone();
                                    let bc_ref = {
                                        let r = git_ref();
                                        if r.is_empty() { String::new() } else { format!("?ref={}", r) }
                                    };
                                    let segments: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
                                    Some(view! {
                                        <nav class="breadcrumb-path mb-2" style="font-size: 0.875rem;">
                                            <a href={format!("/{}/{}{}", bc_owner, bc_repo, bc_ref)}>{bc_repo.clone()}</a>
                                            {segments.iter().enumerate().map(|(i, seg)| {
                                                let seg_path = segments[..=i].join("/");
                                                let href = format!("/{}/{}/tree/{}{}", bc_owner, bc_repo, seg_path, bc_ref);
                                                view! {
                                                    <span class="breadcrumb-sep">" / "</span>
                                                    <a href={href}>{seg.to_string()}</a>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </nav>
                                    })
                                }
                            }

                            // File tree
                            {if resp.entries.is_empty() {
                                let new_repo_url = clone_url_for_instructions.clone();
                                let existing_repo_url = clone_url_for_instructions.clone();
                                let display_repo_name = repo_name.clone();
                                view! {
                                    <div class="setup-instructions">
                                        <div class="setup-section">
                                            <h3 class="setup-heading">"Quick setup"</h3>
                                            <div class="setup-clone-url">
                                                <code>{clone_url_for_instructions}</code>
                                            </div>
                                        </div>
                                        <div class="setup-section">
                                            <h3 class="setup-heading">"…or create a new repository on the command line"</h3>
                                            <pre class="setup-code">{format!("\
echo \"# {}\" >> README.md\n\
git init\n\
git add README.md\n\
git commit -m \"first commit\"\n\
git branch -M main\n\
git remote add origin {}\n\
git push -u origin main", display_repo_name, new_repo_url)}</pre>
                                        </div>
                                        <div class="setup-section">
                                            <h3 class="setup-heading">"…or push an existing repository from the command line"</h3>
                                            <pre class="setup-code">{format!("\
git remote add origin {}\n\
git branch -M main\n\
git push -u origin main", existing_repo_url)}</pre>
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                let owner_for_list = owner_name.clone();
                                let repo_for_list = repo_name.clone();
                                let current_path = path().unwrap_or_default();
                                let base_path = if current_path.is_empty() {
                                    String::new()
                                } else {
                                    format!("{}/", current_path)
                                };
                                let ref_query = {
                                    let r = git_ref();
                                    if r.is_empty() { String::new() } else { format!("?ref={}", r) }
                                };
                                view! {
                                    <div class="card-flush" style="border-top-left-radius: 0; border-top-right-radius: 0;">
                                        <ul class="file-tree">
                                            {resp.entries.into_iter().map(|entry| {
                                                let name = entry.name.clone();
                                                let href = if entry.is_dir {
                                                    format!("/{}/{}/tree/{}{}{}", owner_for_list, repo_for_list, base_path, name, ref_query)
                                                } else {
                                                    format!("/{}/{}/blob/{}{}{}", owner_for_list, repo_for_list, base_path, name, ref_query)
                                                };
                                                let size_str = if entry.is_dir {
                                                    String::new()
                                                } else {
                                                    format_size(entry.size)
                                                };
                                                let is_dir = entry.is_dir;
                                                view! {
                                                    <li class="file-entry">
                                                        <a href={href}>
                                                            <span class={if is_dir { "icon-folder" } else { "icon-file" }}>
                                                                {if is_dir {
                                                                    view! { <IconFolder /> }.into_any()
                                                                } else {
                                                                    view! { <IconFile /> }.into_any()
                                                                }}
                                                            </span>
                                                            <span>{name}</span>
                                                        </a>
                                                        <span class="file-size">{size_str}</span>
                                                    </li>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </ul>
                                    </div>
                                }.into_any()
                            }}

                            // REMIX.md card
                            {resp.remix_html.map(|html| view! {
                                <div class="remix-card mt-4">
                                    <div class="remix-card-header">
                                        <span class="remix-card-title">"Remix this project"</span>
                                        <span class="remix-card-badge">"Remixable"</span>
                                    </div>
                                    <div class="remix-card-content" inner_html={html}></div>
                                </div>
                            })}

                            // README
                            {resp.readme_html.map(|html| view! {
                                <div class="card mt-4">
                                    <div class="card-header">"README.md"</div>
                                    <div class="readme-content" inner_html={html}></div>
                                </div>
                            })}
                        }.into_any()
                    }
                    Err(e) => view! {
                        <div class="flash flash-error">{e.to_string()}</div>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
