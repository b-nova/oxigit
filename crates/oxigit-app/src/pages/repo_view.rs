use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_params_map};

use crate::components::copy_button::CopyButton;
use crate::components::error_display::ErrorDisplay;
use crate::components::icons::{IconFile, IconFolder, IconLock, IconSearch};
use crate::components::loading::{LoadingCard, LoadingPage};

#[allow(unused_imports)]
use super::{CommitSummary, RepoInfo, RepoTreeResponse, TreeEntryInfo};
#[allow(unused_imports)]
use super::ai_hub::{fetch_ai_hub, AiHubResponse};
#[allow(unused_imports)]
use super::commits::{fetch_commits, CommitEntry};
#[allow(unused_imports)]
use super::issue_list::{list_issues, IssueSummary};
#[allow(unused_imports)]
use super::pr_list::{list_prs, PrSummary};
#[allow(unused_imports)]
use super::{AiMetadataInfo, AiTimelineEntry, SessionListItem, SessionListResponse, ViolationInfo};

#[server]
async fn fetch_repo_tree(
    owner: String,
    repo: String,
    git_ref: String,
    path: String,
) -> Result<RepoTreeResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_base_url, get_data_dir, get_repo_pool};
    use oxigit_core::{db, git};

    let pool = get_repo_pool(&owner, &repo).await?;
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
    use crate::server_fns::{extract_session_user, get_data_dir, get_repo_pool};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_repo_pool(&owner, &repo).await?;
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

    let (active_tab, set_active_tab) = signal("code".to_string());

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
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
                                <code class="clone-bar-url">{clone_url.clone()}</code>
                                <CopyButton text=clone_url />
                            </div>

                            // Tab navigation
                            <nav class="repo-nav mb-4">
                                <button
                                    class=move || if active_tab.get() == "code" { "repo-nav-tab repo-nav-tab-active" } else { "repo-nav-tab" }
                                    on:click=move |_| set_active_tab.set("code".to_string())
                                >"Code"</button>
                                <button
                                    class=move || if active_tab.get() == "issues" { "repo-nav-tab repo-nav-tab-active" } else { "repo-nav-tab" }
                                    on:click=move |_| set_active_tab.set("issues".to_string())
                                >"Issues"</button>
                                <button
                                    class=move || if active_tab.get() == "commits" { "repo-nav-tab repo-nav-tab-active" } else { "repo-nav-tab" }
                                    on:click=move |_| set_active_tab.set("commits".to_string())
                                >"Commits"</button>
                                <button
                                    class=move || if active_tab.get() == "pulls" { "repo-nav-tab repo-nav-tab-active" } else { "repo-nav-tab" }
                                    on:click=move |_| set_active_tab.set("pulls".to_string())
                                >"Pull Requests"</button>
                                <button
                                    class=move || if active_tab.get() == "ai" { "repo-nav-tab repo-nav-tab-ai repo-nav-tab-active" } else { "repo-nav-tab repo-nav-tab-ai" }
                                    on:click=move |_| set_active_tab.set("ai".to_string())
                                >"AI"</button>
                            </nav>

                            // Tab content
                            {
                                let code_owner = owner_name.clone();
                                let code_repo = repo_name.clone();
                                let issues_owner = owner_name.clone();
                                let issues_repo = repo_name.clone();
                                let commits_owner = owner_name.clone();
                                let commits_repo = repo_name.clone();
                                let pulls_owner = owner_name.clone();
                                let pulls_repo = repo_name.clone();
                                let ai_owner = owner_name.clone();
                                let ai_repo = repo_name.clone();
                                let code_branches = resp.branches.clone();
                                let code_ref = resp.current_ref.clone();
                                let code_commit = resp.commit.clone();
                                let code_entries = resp.entries.clone();
                                let code_readme = resp.readme_html.clone();
                                let code_remix = resp.remix_html.clone();
                                let code_clone_url = clone_url_for_instructions.clone();
                                let code_path = path().unwrap_or_default();
                                let code_git_ref = git_ref();
                                view! {
                                    <div>
                                        <Show when=move || active_tab.get() == "code">
                                            <CodeTabContent
                                                owner=code_owner.clone()
                                                repo=code_repo.clone()
                                                branches=code_branches.clone()
                                                current_ref=code_ref.clone()
                                                commit=code_commit.clone()
                                                entries=code_entries.clone()
                                                readme_html=code_readme.clone()
                                                remix_html=code_remix.clone()
                                                clone_url=code_clone_url.clone()
                                                path=code_path.clone()
                                                git_ref=code_git_ref.clone()
                                            />
                                        </Show>
                                        <Show when=move || active_tab.get() == "issues">
                                            <IssueTabContent owner=issues_owner.clone() repo=issues_repo.clone() />
                                        </Show>
                                        <Show when=move || active_tab.get() == "commits">
                                            <CommitTabContent owner=commits_owner.clone() repo=commits_repo.clone() />
                                        </Show>
                                        <Show when=move || active_tab.get() == "pulls">
                                            <PrTabContent owner=pulls_owner.clone() repo=pulls_repo.clone() />
                                        </Show>
                                        <Show when=move || active_tab.get() == "ai">
                                            <AiTabContent owner=ai_owner.clone() repo=ai_repo.clone() />
                                        </Show>
                                    </div>
                                }
                            }
                        }.into_any()
                    }
                    Err(e) => view! {
                        <ErrorDisplay error=e.to_string() />
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}

#[component]
fn CodeTabContent(
    owner: String,
    repo: String,
    branches: Vec<String>,
    current_ref: String,
    commit: Option<CommitSummary>,
    entries: Vec<TreeEntryInfo>,
    readme_html: Option<String>,
    remix_html: Option<String>,
    clone_url: String,
    path: String,
    git_ref: String,
) -> impl IntoView {
    let nav_owner = owner.clone();
    let nav_repo = repo.clone();
    let owner_for_list = owner.clone();
    let repo_for_list = repo.clone();
    let bc_owner = owner.clone();
    let bc_repo = repo.clone();
    let clone_url_for_instructions = clone_url.clone();
    let display_repo_name = repo.clone();
    let path_for_breadcrumb = path.clone();
    let path_for_tree = path.clone();

    view! {
        // Branch selector
        {if !branches.is_empty() {
            let cr = current_ref.clone();
            Some(view! {
                <div class="flex-row gap-2 mb-4">
                    <select
                        class="branch-select"
                        on:change=move |ev| {
                            let selected = event_target_value(&ev);
                            let url = if path.is_empty() {
                                format!("/{}/{}?ref={}", nav_owner, nav_repo, selected)
                            } else {
                                format!("/{}/{}/tree/{}?ref={}", nav_owner, nav_repo, path, selected)
                            };
                            let _ = window().location().set_href(&url);
                        }
                    >
                        {branches.iter().map(|b| {
                            let is_selected = *b == cr;
                            view! { <option value={b.clone()} selected=is_selected>{b.clone()}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>
            })
        } else {
            None
        }}

        // Latest commit
        {commit.map(|c| {
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
            let current_path = path_for_breadcrumb;
            if current_path.is_empty() {
                None
            } else {
                let bc_ref = if git_ref.is_empty() { String::new() } else { format!("?ref={}", git_ref) };
                let segments: Vec<String> = current_path.split('/').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                let bc_repo_clone = bc_repo.clone();
                Some(view! {
                    <nav class="breadcrumb-path mb-2" style="font-size: 0.875rem;">
                        <a href={format!("/{}/{}{}", bc_owner, bc_repo, bc_ref)}>{bc_repo_clone}</a>
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
        {if entries.is_empty() {
            let new_repo_url = clone_url_for_instructions.clone();
            let existing_repo_url = clone_url_for_instructions.clone();
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
            let base_path = if path_for_tree.is_empty() {
                String::new()
            } else {
                format!("{}/", path_for_tree)
            };
            let ref_query = if git_ref.is_empty() { String::new() } else { format!("?ref={}", git_ref) };
            view! {
                <div class="card-flush" style="border-top-left-radius: 0; border-top-right-radius: 0;">
                    <ul class="file-tree">
                        {entries.into_iter().map(|entry| {
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
        {remix_html.map(|html| view! {
            <div class="remix-card mt-4">
                <div class="remix-card-header">
                    <span class="remix-card-title">"Remix this project"</span>
                    <span class="remix-card-badge">"Remixable"</span>
                </div>
                <div class="remix-card-content" inner_html={html}></div>
            </div>
        })}

        // README
        {readme_html.map(|html| view! {
            <div class="card mt-4">
                <div class="card-header">"README.md"</div>
                <div class="readme-content" inner_html={html}></div>
            </div>
        })}
    }
}

#[component]
fn IssueTabContent(owner: String, repo: String) -> impl IntoView {
    let owner_c = owner.clone();
    let repo_c = repo.clone();
    let (status_filter, set_status_filter) = signal("open".to_string());

    let issues = Resource::new(
        move || (owner.clone(), repo.clone(), status_filter.get()),
        move |(owner, repo, status)| list_issues(owner, repo, status),
    );

    view! {
        <div class="flex-row-between gap-2 mb-4">
            <div class="filter-tabs">
                <button
                    class=move || if status_filter.get() == "open" { "filter-tab filter-tab-active" } else { "filter-tab" }
                    on:click=move |_| set_status_filter.set("open".to_string())
                >"Open"</button>
                <button
                    class=move || if status_filter.get() == "closed" { "filter-tab filter-tab-active" } else { "filter-tab" }
                    on:click=move |_| set_status_filter.set("closed".to_string())
                >"Closed"</button>
            </div>
            <a href={format!("/{}/{}/issues/new", owner_c, repo_c)} class="btn btn-primary btn-sm">"New Issue"</a>
        </div>
        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
                {move || {
                    let owner_name = owner_c.clone();
                    let repo_name = repo_c.clone();
                    let current_status = status_filter.get();
                    Suspend::new(async move {
                        match issues.await {
                            Ok(issues) if issues.is_empty() => {
                                let msg = format!("No {} issues.", current_status);
                                view! {
                                    <div class="empty-state">{msg}</div>
                                }.into_any()
                            },
                            Ok(issues) => view! {
                                <ul class="list">
                                    {issues.into_iter().map(|issue| {
                                        let href = format!("/{}/{}/issues/{}", owner_name, repo_name, issue.number);
                                        let title = issue.title.clone();
                                        let number = issue.number;
                                        let author = issue.author.clone();
                                        let created = issue.created_at.clone();
                                        let badge_class = if issue.status == "open" { "badge badge-open" } else { "badge badge-closed" };
                                        view! {
                                            <li class="list-item">
                                                <div class="flex-1">
                                                    <div class="flex-row gap-2">
                                                        <a href={href} class="font-semibold">{title}</a>
                                                        <span class="text-tertiary">{"#"}{number}</span>
                                                    </div>
                                                    <div class="list-item-meta mt-1">
                                                        <span class={badge_class}>{issue.status.clone()}</span>
                                                        " opened by " {author} " on " {created}
                                                    </div>
                                                </div>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any(),
                            Err(e) => view! {
                                <ErrorDisplay error=e.to_string() />
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn CommitTabContent(owner: String, repo: String) -> impl IntoView {
    let owner_c = owner.clone();
    let repo_c = repo.clone();

    let commits = Resource::new(
        move || (owner.clone(), repo.clone()),
        move |(owner, repo)| fetch_commits(owner, repo),
    );

    view! {
        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
                {move || {
                    let owner_name = owner_c.clone();
                    let repo_name = repo_c.clone();
                    Suspend::new(async move {
                        match commits.await {
                            Ok(commits) if commits.is_empty() => view! {
                                <div class="empty-state">"No commits yet."</div>
                            }.into_any(),
                            Ok(commits) => view! {
                                <ul class="list">
                                    {commits.into_iter().map(|c| {
                                        let sha = c.id.clone();
                                        let short = c.short_id.clone();
                                        let msg = c.message.clone();
                                        let author = c.author.clone();
                                        let time = c.time.clone();
                                        let ai_tool = c.ai_tool.clone();
                                        let href = format!("/{}/{}/commit/{}", owner_name, repo_name, sha);
                                        view! {
                                            <li class="commit-item">
                                                <div class="flex-1">
                                                    <div class="commit-message">
                                                        <a href={href} style="color: var(--text);">{msg}</a>
                                                        {ai_tool.map(|tool| view! {
                                                            <span class="ai-badge ai-badge-sm">{tool}</span>
                                                        })}
                                                    </div>
                                                    <div class="commit-meta">
                                                        {author} " committed " {time}
                                                    </div>
                                                </div>
                                                <span class="commit-sha">{short}</span>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any(),
                            Err(e) => view! {
                                <ErrorDisplay error=e.to_string() />
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn PrTabContent(owner: String, repo: String) -> impl IntoView {
    let owner_c = owner.clone();
    let repo_c = repo.clone();
    let (status_filter, set_status_filter) = signal("open".to_string());

    let prs = Resource::new(
        move || (owner.clone(), repo.clone(), status_filter.get()),
        move |(owner, repo, status)| list_prs(owner, repo, status),
    );

    view! {
        <div class="flex-row-between gap-2 mb-4">
            <div class="filter-tabs">
                <button
                    class=move || if status_filter.get() == "open" { "filter-tab filter-tab-active" } else { "filter-tab" }
                    on:click=move |_| set_status_filter.set("open".to_string())
                >"Open"</button>
                <button
                    class=move || if status_filter.get() == "merged" { "filter-tab filter-tab-active" } else { "filter-tab" }
                    on:click=move |_| set_status_filter.set("merged".to_string())
                >"Merged"</button>
                <button
                    class=move || if status_filter.get() == "closed" { "filter-tab filter-tab-active" } else { "filter-tab" }
                    on:click=move |_| set_status_filter.set("closed".to_string())
                >"Closed"</button>
            </div>
            <a href={format!("/{}/{}/pulls/new", owner_c, repo_c)} class="btn btn-primary btn-sm">"New Pull Request"</a>
        </div>
        <div class="card-flush">
            <Suspense fallback=|| view! { <LoadingCard /> }>
                {move || {
                    let owner_name = owner_c.clone();
                    let repo_name = repo_c.clone();
                    let current_status = status_filter.get();
                    Suspend::new(async move {
                        match prs.await {
                            Ok(prs) if prs.is_empty() => {
                                let msg = format!("No {} pull requests.", current_status);
                                view! {
                                    <div class="empty-state">{msg}</div>
                                }.into_any()
                            },
                            Ok(prs) => view! {
                                <ul class="list">
                                    {prs.into_iter().map(|pr| {
                                        let href = format!("/{}/{}/pulls/{}", owner_name, repo_name, pr.number);
                                        let title = pr.title.clone();
                                        let number = pr.number;
                                        let author = pr.author.clone();
                                        let created = pr.created_at.clone();
                                        let badge_class = match pr.status.as_str() {
                                            "open" => "badge badge-open",
                                            "merged" => "badge badge-merged",
                                            "closed" => "badge badge-closed",
                                            _ => "badge",
                                        };
                                        let target = pr.target_branch.clone();
                                        let source = pr.source_branch.clone();
                                        view! {
                                            <li class="list-item">
                                                <div class="flex-1">
                                                    <div class="flex-row gap-2">
                                                        <a href={href} class="font-semibold">{title}</a>
                                                        <span class="text-tertiary">{"#"}{number}</span>
                                                    </div>
                                                    <div class="list-item-meta mt-1 flex-row gap-2" style="flex-wrap: wrap;">
                                                        <span class={badge_class}>{pr.status.clone()}</span>
                                                        <span>{author}</span>
                                                        <span>
                                                            <span class="badge-branch">{target}</span>
                                                            " \u{2190} "
                                                            <span class="badge-branch">{source}</span>
                                                        </span>
                                                        <span>{created}</span>
                                                    </div>
                                                </div>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_any(),
                            Err(e) => view! {
                                <ErrorDisplay error=e.to_string() />
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn AiTabContent(owner: String, repo: String) -> impl IntoView {
    let owner_c = owner.clone();
    let repo_c = repo.clone();
    let (query, set_query) = signal(String::new());

    let hub = Resource::new(
        move || (owner.clone(), repo.clone(), query.get()),
        move |(owner, repo, query)| fetch_ai_hub(owner, repo, query),
    );

    view! {
        <div class="search-wrapper mb-4">
            <IconSearch />
            <input
                type="text"
                class="search-input"
                placeholder="Search prompts..."
                on:input=move |ev| {
                    set_query.set(event_target_value(&ev));
                }
            />
        </div>
        <Suspense fallback=|| view! { <LoadingCard /> }>
            {move || {
                let owner_name = owner_c.clone();
                let repo_name = repo_c.clone();
                let has_query = !query.get().is_empty();
                Suspend::new(async move {
                    match hub.await {
                        Ok(resp) if resp.sessions.is_empty() && resp.unsessioned.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">
                                    {if has_query { "No matching sessions." } else { "No AI activity yet." }}
                                </p>
                                {(!has_query).then(|| view! {
                                    <p class="empty-state-text">
                                        "Push commits with AI metadata to see sessions and conversation history here."
                                    </p>
                                })}
                            </div>
                        }.into_any(),
                        Ok(resp) => {
                            let mut parts: Vec<AnyView> = Vec::new();

                            // Sessions grouped by tool
                            if !resp.sessions.is_empty() {
                                let mut tool_order: Vec<String> = Vec::new();
                                let mut groups: std::collections::HashMap<String, Vec<SessionListItem>> = std::collections::HashMap::new();
                                for s in resp.sessions {
                                    if !groups.contains_key(&s.ai_tool) {
                                        tool_order.push(s.ai_tool.clone());
                                    }
                                    groups.entry(s.ai_tool.clone()).or_default().push(s);
                                }

                                for tool_name in tool_order {
                                    let sessions = groups.remove(&tool_name).unwrap_or_default();
                                    let count = sessions.len();
                                    let cards: Vec<AnyView> = sessions.into_iter().map(|s| {
                                        let href = format!("/{}/{}/ai/{}", owner_name, repo_name, s.session_id);
                                        let short_id = s.session_id[..8.min(s.session_id.len())].to_string();
                                        let prompt_preview = s.first_prompt
                                            .map(|p| if p.len() > 100 { format!("{}...", &p[..100]) } else { p })
                                            .unwrap_or_default();
                                        view! {
                                            <a href={href} class="session-card">
                                                <div class="session-card-header">
                                                    <span class="session-card-id">{short_id}</span>
                                                    <span class="text-tertiary">
                                                        {s.commit_count} " commit" {if s.commit_count != 1 { "s" } else { "" }}
                                                    </span>
                                                </div>
                                                {(!prompt_preview.is_empty()).then(|| view! {
                                                    <p class="session-card-prompt">{prompt_preview}</p>
                                                })}
                                                <div class="session-card-time">
                                                    {s.first_time} " — " {s.last_time}
                                                </div>
                                            </a>
                                        }.into_any()
                                    }).collect();
                                    parts.push(view! {
                                        <div class="tool-group-heading">
                                            <span class="ai-badge">{tool_name}</span>
                                            <span class="tool-group-count">
                                                {count} " session" {if count != 1 { "s" } else { "" }}
                                            </span>
                                        </div>
                                        <div class="session-list mb-4">{cards}</div>
                                    }.into_any());
                                }
                            }

                            // Unsessioned commits
                            if !resp.unsessioned.is_empty() {
                                let items: Vec<AnyView> = resp.unsessioned.into_iter().map(|entry| {
                                    let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, entry.commit_sha);
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="flex-row gap-2">
                                                    <span class="ai-badge">{entry.metadata.ai_tool}</span>
                                                    <a href={commit_href} class="commit-sha">{entry.short_sha}</a>
                                                    <span>{entry.commit_message}</span>
                                                </div>
                                                {entry.metadata.ai_prompt.map(|p| view! {
                                                    <p class="list-item-desc">{p}</p>
                                                })}
                                            </div>
                                            <span class="list-item-meta">{entry.commit_time}</span>
                                        </li>
                                    }.into_any()
                                }).collect();
                                parts.push(view! {
                                    <div class="card-flush">
                                        <div class="card-header">"Individual AI commits"</div>
                                        <ul class="list">{items}</ul>
                                    </div>
                                }.into_any());
                            }

                            // Guardrail violations
                            if !resp.violations.is_empty() {
                                let violation_items: Vec<AnyView> = resp.violations.into_iter().map(|v| {
                                    let badge_class = if v.action_taken == "blocked" {
                                        "violation-badge violation-blocked"
                                    } else {
                                        "violation-badge violation-warned"
                                    };
                                    let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, v.commit_sha);
                                    view! {
                                        <div class="violation-item">
                                            <span class={badge_class}>{v.action_taken}</span>
                                            <span class="violation-badge" style="background: var(--bg-tertiary);">{v.category}</span>
                                            <span class="violation-item-msg">{v.message}</span>
                                            <a href={commit_href} class="commit-sha">{v.short_sha}</a>
                                            {v.file_path.map(|f| view! { <span class="violation-item-file">{f}</span> })}
                                        </div>
                                    }.into_any()
                                }).collect();
                                parts.push(view! {
                                    <div class="card-flush mt-4">
                                        <div class="card-header">"Guardrail Violations"</div>
                                        <div>{violation_items}</div>
                                    </div>
                                }.into_any());
                            }

                            view! { <div>{parts}</div> }.into_any()
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

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
