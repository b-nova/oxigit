use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::IconSearch;
use crate::components::loading::LoadingCard;

use super::{PromptHistoryEntry, PromptHistoryResponse};

#[server]
async fn fetch_prompt_history(
    owner: String,
    repo: String,
    query: String,
    page: i64,
) -> Result<PromptHistoryResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_ai_access_level, get_data_dir, get_repo_pool};
    use oxigit_core::{db, entitlements::AiAccessLevel, git};
    use super::PromptCommitInfo;

    const PAGE_SIZE: i64 = 20;
    const FREE_LIMIT: i64 = 10;

    let pool = get_repo_pool(&owner, &repo).await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let ai_access = get_ai_access_level(current_user.id).await?;
    let is_limited = ai_access == AiAccessLevel::Limited;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, Some(current_user.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);
    let (effective_page_size, offset) = if is_limited {
        (FREE_LIMIT, 0i64)
    } else {
        (PAGE_SIZE, page * PAGE_SIZE)
    };

    let total_prompts = db::count_prompt_groups(&pool, repo_db.id, &query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let groups = db::list_prompt_groups(&pool, repo_db.id, &query, effective_page_size, offset)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let entries: Vec<PromptHistoryEntry> = groups
        .into_iter()
        .map(|g| {
            let commits: Vec<PromptCommitInfo> = g
                .commit_shas
                .iter()
                .map(|sha| {
                    let commit_info = git::get_latest_commit(&repo_path, sha).unwrap_or(None);
                    let (message, author, time) = match commit_info {
                        Some(c) => (c.message, c.author, c.time),
                        None => ("(commit not found)".into(), String::new(), String::new()),
                    };
                    PromptCommitInfo {
                        short_sha: sha[..7.min(sha.len())].to_string(),
                        sha: sha.clone(),
                        message,
                        author,
                        time,
                    }
                })
                .collect();

            PromptHistoryEntry {
                prompt_text: g.ai_prompt,
                session_id: if is_limited { None } else { g.ai_session_id },
                prompt_index: g.ai_prompt_index,
                ai_tool: g.ai_tool,
                ai_model: g.ai_model,
                commit_count: g.commit_count,
                commits,
                first_time: g.first_time,
                last_time: g.last_time,
            }
        })
        .collect();

    let has_more = if is_limited {
        total_prompts > FREE_LIMIT
    } else {
        (offset + PAGE_SIZE) < total_prompts
    };

    Ok(PromptHistoryResponse {
        entries,
        total_prompts,
        has_more,
        is_limited,
    })
}

#[component]
pub fn PromptHistoryPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let (query, set_query) = signal(String::new());
    let (page, set_page) = signal(0i64);

    let data = Resource::new(
        move || (owner(), repo(), query.get(), page.get()),
        move |(owner, repo, query, page)| fetch_prompt_history(owner, repo, query, page),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"Prompts"</span>
            </h1>
            <a href={move || format!("/{}/{}/ai", owner(), repo())} class="btn btn-sm">
                "AI Hub"
            </a>
        </div>
        <div class="search-wrapper">
            <IconSearch />
            <input
                type="text"
                class="search-input"
                placeholder="Search prompts... e.g. 'add dark mode'"
                on:input=move |ev| {
                    set_query.set(event_target_value(&ev));
                    set_page.set(0);
                }
            />
        </div>
        <Suspense fallback=|| view! { <LoadingCard /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                let has_query = !query.get().is_empty();
                let current_page = page.get();
                Suspend::new(async move {
                    match data.await {
                        Ok(resp) if resp.entries.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">
                                    {if has_query { "No matching prompts." } else { "No AI prompts yet." }}
                                </p>
                                {(!has_query).then(|| view! {
                                    <p class="empty-state-text">
                                        "Push commits with AI metadata to see your prompt history here."
                                    </p>
                                })}
                            </div>
                        }.into_any(),
                        Ok(resp) => {
                            let total = resp.total_prompts;
                            let has_more = resp.has_more;
                            let limited = resp.is_limited;
                            let cards: Vec<_> = resp.entries.into_iter().map(|entry| {
                                render_prompt_card(&owner_name, &repo_name, entry)
                            }).collect();

                            view! {
                                <p class="text-tertiary mb-3">
                                    {total} " prompt" {if total != 1 { "s" } else { "" }}
                                </p>
                                <div class="prompt-timeline">{cards}</div>
                                {if limited && has_more {
                                    view! {
                                        <div class="flash flash-info mt-4">
                                            "Showing 10 most recent prompts. "
                                            <a href="/pricing">"Upgrade to Flat"</a>
                                            " for full prompt history."
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="pagination">
                                            {(current_page > 0).then(|| view! {
                                                <button class="btn btn-sm"
                                                    on:click=move |_| set_page.set(current_page - 1)>
                                                    "Previous"
                                                </button>
                                            })}
                                            {has_more.then(|| view! {
                                                <button class="btn btn-sm"
                                                    on:click=move |_| set_page.set(current_page + 1)>
                                                    "Next"
                                                </button>
                                            })}
                                        </div>
                                    }.into_any()
                                }}
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

fn render_prompt_card(owner: &str, repo: &str, entry: PromptHistoryEntry) -> impl IntoView {
    let prompt_display = entry
        .prompt_text
        .clone()
        .unwrap_or_else(|| "(no prompt recorded)".to_string());

    let session_link = entry.session_id.as_ref().map(|sid| {
        let href = format!("/{}/{}/ai/{}", owner, repo, sid);
        let short_id = sid[..8.min(sid.len())].to_string();
        (href, short_id)
    });

    let commit_views: Vec<_> = entry
        .commits
        .iter()
        .map(|c| {
            let href = format!("/{}/{}/commit/{}", owner, repo, c.sha);
            view! {
                <li class="prompt-commit-item">
                    <a href={href} class="commit-sha">{c.short_sha.clone()}</a>
                    <span class="prompt-commit-msg">{c.message.clone()}</span>
                    <span class="text-tertiary">{c.time.clone()}</span>
                </li>
            }
        })
        .collect();

    view! {
        <div class="prompt-card card">
            <div class="prompt-card-header">
                <div class="prompt-card-text">{prompt_display}</div>
                <div class="prompt-card-meta">
                    <span class="ai-badge">{entry.ai_tool}</span>
                    {entry.ai_model.map(|m| view! { <span class="tag">{m}</span> })}
                    <span class="text-tertiary">
                        {entry.commit_count} " commit" {if entry.commit_count != 1 { "s" } else { "" }}
                    </span>
                    {session_link.map(|(href, short_id)| view! {
                        <a href={href} class="tag tag-link">{"session "} {short_id}</a>
                    })}
                </div>
            </div>
            <details class="prompt-commits-details">
                <summary class="prompt-commits-summary">
                    "Show commits"
                </summary>
                <ul class="prompt-commits-list">{commit_views}</ul>
            </details>
            <div class="prompt-card-time">
                {
                    let show_range = entry.first_time != entry.last_time;
                    let last = entry.last_time.clone();
                    view! {
                        {entry.first_time}
                        {show_range.then(|| view! {
                            <span>" — " {last}</span>
                        })}
                    }
                }
            </div>
        </div>
    }
}
