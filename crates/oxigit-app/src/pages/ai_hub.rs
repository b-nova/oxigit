use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::icons::IconSearch;
use crate::components::loading::LoadingCard;

#[allow(unused_imports)]
use super::{AiMetadataInfo, AiTimelineEntry, SessionListItem, SessionListResponse, ViolationInfo};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiHubResponse {
    pub sessions: Vec<SessionListItem>,
    pub unsessioned: Vec<AiTimelineEntry>,
    pub violations: Vec<ViolationInfo>,
    #[serde(default)]
    pub drill_down_enabled: bool,
}

#[server]
pub async fn fetch_ai_hub(
    owner: String,
    repo: String,
    query: String,
) -> Result<AiHubResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_ai_access_level, get_repo_path, get_repo_pools};
    use oxigit_core::{db, entitlements::AiAccessLevel, git};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let ai_access = get_ai_access_level(current_user.id).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, Some(current_user.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;

    // Sessions
    let summaries = db::list_session_summaries(&pool, repo_db.id, &query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let sessions = summaries
        .into_iter()
        .map(|s| SessionListItem {
            session_id: s.session_id,
            ai_tool: s.ai_tool,
            commit_count: s.commit_count,
            first_prompt: s.first_prompt,
            first_time: s.first_time,
            last_time: s.last_time,
            vibe_score: None,
            vibe_grade: None,
        })
        .collect();

    // Unsessioned commits
    let unsessioned_metas = db::get_unsessioned_ai_commits(&pool, repo_db.id, &query)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let unsessioned = unsessioned_metas
        .into_iter()
        .map(|meta| {
            let commit_info = git::get_latest_commit(&repo_path, &meta.commit_sha).unwrap_or(None);
            let (message, author, time) = match commit_info {
                Some(c) => (c.message, c.author, c.time),
                None => ("(commit not found)".into(), String::new(), String::new()),
            };
            AiTimelineEntry {
                short_sha: meta.commit_sha[..7.min(meta.commit_sha.len())].to_string(),
                commit_sha: meta.commit_sha,
                commit_message: message,
                commit_author: author,
                commit_time: time,
                metadata: AiMetadataInfo {
                    ai_tool: meta.ai_tool,
                    ai_model: meta.ai_model,
                    ai_prompt: meta.ai_prompt,
                    ai_session_id: None,
                    ai_files_touched: meta.ai_files_touched.and_then(|f| serde_json::from_str(&f).ok()),
                    ai_prompt_index: meta.ai_prompt_index,
                },
                diff_html: None,
            }
        })
        .collect();

    // Fetch recent guardrail violations
    let violation_records = db::list_guardrail_violations(&pool, repo_db.id, 10)
        .await
        .unwrap_or_default();
    let violations: Vec<ViolationInfo> = violation_records.into_iter().map(|v| {
        let short_sha = v.commit_sha[..7.min(v.commit_sha.len())].to_string();
        ViolationInfo {
            id: v.id,
            commit_sha: v.commit_sha,
            short_sha,
            category: v.rule_category,
            action_taken: v.action_taken,
            severity: v.severity,
            message: v.message,
            file_path: v.file_path,
            created_at: v.created_at,
        }
    }).collect();

    Ok(AiHubResponse {
        sessions,
        unsessioned,
        violations,
        drill_down_enabled: ai_access == AiAccessLevel::Full,
    })
}

#[component]
pub fn AiHubPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let (query, set_query) = signal(String::new());

    let hub = Resource::new(
        move || (owner(), repo(), query.get()),
        move |(owner, repo, query)| fetch_ai_hub(owner, repo, query),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"AI"</span>
            </h1>
        </div>
        <div class="search-wrapper">
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
                let owner_name = owner();
                let repo_name = repo();
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
                            let drill_down = resp.drill_down_enabled;
                            let mut parts: Vec<AnyView> = Vec::new();

                            // Preview banner for free users
                            if !drill_down {
                                parts.push(view! {
                                    <div class="flash flash-info mb-4">
                                        "Viewing AI Hub in preview mode. "
                                        <a href="/pricing">"Upgrade to Flat"</a>
                                        " for full session drill-down."
                                    </div>
                                }.into_any());
                            }

                            // Sessions grouped by tool
                            if !resp.sessions.is_empty() {
                                // Group sessions by ai_tool, preserving order
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
                                        if drill_down {
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
                                        } else {
                                            view! {
                                                <div class="session-card session-card-locked">
                                                    <div class="session-card-header">
                                                        <span class="session-card-id">{short_id}</span>
                                                        <span class="upgrade-badge">"Flat"</span>
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
                                                </div>
                                            }.into_any()
                                        }
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
                                                {(drill_down).then(|| entry.metadata.ai_prompt.clone()).flatten().map(|p| view! {
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
