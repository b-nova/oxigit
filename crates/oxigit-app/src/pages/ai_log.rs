use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiLogEntry {
    pub commit_sha: String,
    pub short_sha: String,
    pub commit_message: String,
    pub commit_time: String,
    pub ai_tool: String,
    pub ai_model: Option<String>,
    pub ai_prompt: Option<String>,
    pub ai_session_id: Option<String>,
    pub files_changed: Vec<String>,
    pub is_session_break: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiLogResponse {
    pub entries: Vec<AiLogEntry>,
}

#[server]
async fn fetch_ai_log(
    owner: String,
    repo: String,
) -> Result<AiLogResponse, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_data_dir, get_pool};
    use oxigit_core::{db, git};

    let pool = get_pool().await?;
    let data_dir = get_data_dir().await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = git::repo_path(&data_dir, &owner, &repo);

    // Fetch all AI metadata ordered chronologically
    let all_metadata = sqlx::query_as::<_, oxigit_core::models::AiCommitMetadata>(
        "SELECT * FROM ai_commit_metadata WHERE repo_id = ? ORDER BY created_at ASC",
    )
    .bind(repo_db.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut entries = Vec::new();
    let mut prev_session: Option<String> = None;

    for meta in all_metadata {
        let is_session_break = match (&prev_session, &meta.ai_session_id) {
            (Some(prev), Some(curr)) => prev != curr,
            (None, Some(_)) => !entries.is_empty(),
            (Some(_), None) => true,
            (None, None) => false,
        };
        prev_session = meta.ai_session_id.clone();

        let commit_info = git::get_latest_commit(&repo_path, &meta.commit_sha).unwrap_or(None);
        let (message, time) = match commit_info {
            Some(c) => (c.message, c.time),
            None => ("(commit not found)".into(), String::new()),
        };

        let files: Vec<String> = meta
            .ai_files_touched
            .as_ref()
            .and_then(|f| serde_json::from_str(f).ok())
            .unwrap_or_default();

        entries.push(AiLogEntry {
            short_sha: meta.commit_sha[..7.min(meta.commit_sha.len())].to_string(),
            commit_sha: meta.commit_sha,
            commit_message: message,
            commit_time: time,
            ai_tool: meta.ai_tool,
            ai_model: meta.ai_model,
            ai_prompt: meta.ai_prompt,
            ai_session_id: meta.ai_session_id,
            files_changed: files,
            is_session_break,
        });
    }

    Ok(AiLogResponse { entries })
}

#[component]
pub fn AiLogPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let log = Resource::new(
        move || (owner(), repo()),
        move |(owner, repo)| fetch_ai_log(owner, repo),
    );

    view! {
        <div class="page-header">
            <h1 class="breadcrumb">
                <a href={move || format!("/{}/{}", owner(), repo())}>
                    {move || owner()} <span class="breadcrumb-sep">" / "</span> {move || repo()}
                </a>
                <span class="breadcrumb-sep">" / "</span>
                <span>"AI Log"</span>
            </h1>
        </div>
        <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match log.await {
                        Ok(resp) if resp.entries.is_empty() => view! {
                            <div class="empty-state card">
                                <p class="empty-state-title">"No AI conversation history yet."</p>
                                <p class="empty-state-text">
                                    "Push commits with AI metadata to see the full pair programming log."
                                </p>
                            </div>
                        }.into_any(),
                        Ok(resp) => {
                            let items: Vec<AnyView> = resp.entries.into_iter().map(|entry| {
                                let commit_href = format!("/{}/{}/commit/{}", owner_name, repo_name, entry.commit_sha);
                                let mut parts: Vec<AnyView> = Vec::new();

                                if entry.is_session_break {
                                    parts.push(view! {
                                        <div class="chat-session-break">
                                            <span class="chat-session-break-line"></span>
                                            <span class="chat-session-break-text">"new session"</span>
                                            <span class="chat-session-break-line"></span>
                                        </div>
                                    }.into_any());
                                }

                                // Prompt bubble (user message)
                                if let Some(ref prompt) = entry.ai_prompt {
                                    parts.push(view! {
                                        <div class="chat-message chat-user">
                                            <div class="chat-message-header">
                                                <span class="chat-sender">"You"</span>
                                                <span class="chat-tool">" → " {entry.ai_tool.clone()}</span>
                                                <span class="chat-time">{entry.commit_time.clone()}</span>
                                            </div>
                                            <div class="chat-bubble chat-bubble-user">{prompt.clone()}</div>
                                        </div>
                                    }.into_any());
                                }

                                // Commit bubble (AI response)
                                let files_str = if entry.files_changed.is_empty() {
                                    String::new()
                                } else {
                                    entry.files_changed.join(", ")
                                };
                                parts.push(view! {
                                    <div class="chat-message chat-ai">
                                        <div class="chat-message-header">
                                            <span class="ai-badge">{entry.ai_tool.clone()}</span>
                                            <span class="chat-time">{entry.commit_time.clone()}</span>
                                        </div>
                                        <div class="chat-bubble chat-bubble-ai">
                                            <div class="chat-commit-line">
                                                <a href={commit_href} class="commit-sha">{entry.short_sha}</a>
                                                " "
                                                {entry.commit_message}
                                            </div>
                                            {(!files_str.is_empty()).then(|| view! {
                                                <div class="chat-files">{files_str}</div>
                                            })}
                                        </div>
                                    </div>
                                }.into_any());

                                view! { <div>{parts}</div> }.into_any()
                            }).collect();
                            view! {
                                <div class="chat-log">{items}</div>
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
