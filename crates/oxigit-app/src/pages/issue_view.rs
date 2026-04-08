use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;
use crate::components::toast::use_toast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueDetail {
    pub number: i64,
    pub title: String,
    pub description: String,
    pub author: String,
    pub status: String,
    pub created_at: String,
    pub comments: Vec<CommentInfo>,
    pub can_manage: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommentInfo {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[server]
async fn get_issue(
    owner: String,
    repo: String,
    number: i64,
) -> Result<IssueDetail, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let pool = get_pool().await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository(&pool, &owner, &repo)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !db::can_access_repo(&repo_db, current_user.as_ref().map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let issue = db::get_issue(&pool, repo_db.id, number)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let author = db::get_user_by_id(&pool, issue.author_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let comments = db::list_issue_comments(&pool, issue.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .into_iter()
        .map(|(c, u)| CommentInfo {
            author: u.username,
            body: c.body,
            created_at: c.created_at,
        })
        .collect();

    let can_manage = current_user
        .map(|u| u.id == repo_db.owner_id || u.id == issue.author_id)
        .unwrap_or(false);

    Ok(IssueDetail {
        number: issue.number,
        title: issue.title,
        description: issue.description,
        author: author.username,
        status: issue.status,
        created_at: issue.created_at,
        comments,
        can_manage,
    })
}

#[server]
async fn close_issue_action(owner: String, repo: String, number: i64) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user().await.ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let (_, repo_db) = db::get_repository(&pool, &owner, &repo).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    let issue = db::get_issue(&pool, repo_db.id, number).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if user.id != repo_db.owner_id && user.id != issue.author_id {
        return Err(ServerFnError::new("Not authorized"));
    }

    db::close_issue(&pool, repo_db.id, number).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    leptos_axum::redirect(&format!("/{}/{}/issues/{}", owner, repo, number));
    Ok(())
}

#[server]
async fn reopen_issue_action(owner: String, repo: String, number: i64) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user().await.ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let (_, repo_db) = db::get_repository(&pool, &owner, &repo).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    let issue = db::get_issue(&pool, repo_db.id, number).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if user.id != repo_db.owner_id && user.id != issue.author_id {
        return Err(ServerFnError::new("Not authorized"));
    }

    db::reopen_issue(&pool, repo_db.id, number).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    leptos_axum::redirect(&format!("/{}/{}/issues/{}", owner, repo, number));
    Ok(())
}

#[server]
async fn add_comment(owner: String, repo: String, number: i64, body: String) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;

    let user = extract_session_user().await.ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let (_, repo_db) = db::get_repository(&pool, &owner, &repo).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    let issue = db::get_issue(&pool, repo_db.id, number).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    db::add_issue_comment(&pool, issue.id, user.id, &body).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    leptos_axum::redirect(&format!("/{}/{}/issues/{}", owner, repo, number));
    Ok(())
}

#[component]
pub fn IssueViewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let number = move || params.read().get("number").and_then(|n| n.parse::<i64>().ok()).unwrap_or(0);

    let issue = Resource::new(
        move || (owner(), repo(), number()),
        move |(o, r, n)| get_issue(o, r, n),
    );

    let close_action = ServerAction::<CloseIssueAction>::new();
    let reopen_action = ServerAction::<ReopenIssueAction>::new();
    let comment_action = ServerAction::<AddComment>::new();

    let toast = use_toast();
    let toast_close = toast.clone();
    let toast_reopen = toast.clone();
    let toast_comment = toast.clone();
    Effect::new(move |_| {
        if let Some(Ok(_)) = close_action.value().get() {
            toast_close.success("Issue closed");
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(_)) = reopen_action.value().get() {
            toast_reopen.success("Issue reopened");
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(_)) = comment_action.value().get() {
            toast_comment.success("Comment added");
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                let num = number();
                Suspend::new(async move {
                    match issue.await {
                        Ok(detail) => {
                            let badge_class = if detail.status == "open" { "badge badge-open" } else { "badge badge-closed" };

                            view! {
                                <div class="page-header">
                                    <h1 class="page-title-sm">
                                        {detail.title.clone()}
                                        <span class="text-secondary" style="font-weight: 400;"> " #" {num}</span>
                                    </h1>
                                </div>

                                <div class="flex-row gap-3 mb-4">
                                    <span class={badge_class}>{detail.status.clone()}</span>
                                    <span class="text-secondary" style="font-size: 0.875rem;">
                                        {detail.author.clone()} " opened this issue " {detail.created_at.clone()}
                                    </span>
                                </div>

                                // Description
                                {(!detail.description.is_empty()).then(|| {
                                    let desc = detail.description.clone();
                                    view! {
                                        <div class="card mb-4">
                                            <p class="whitespace-pre">{desc}</p>
                                        </div>
                                    }
                                })}

                                // Close/Reopen buttons
                                {detail.can_manage.then(|| {
                                    let o = owner_name.clone();
                                    let r = repo_name.clone();
                                    if detail.status == "open" {
                                        view! {
                                            <ActionForm action=close_action>
                                                <input type="hidden" name="owner" value={o} />
                                                <input type="hidden" name="repo" value={r} />
                                                <input type="hidden" name="number" value={num.to_string()} />
                                                <button type="submit" class="btn btn-danger mb-4">"Close Issue"</button>
                                            </ActionForm>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <ActionForm action=reopen_action>
                                                <input type="hidden" name="owner" value={o} />
                                                <input type="hidden" name="repo" value={r} />
                                                <input type="hidden" name="number" value={num.to_string()} />
                                                <button type="submit" class="btn btn-primary mb-4">"Reopen Issue"</button>
                                            </ActionForm>
                                        }.into_any()
                                    }
                                })}

                                // Comments
                                {(!detail.comments.is_empty()).then(|| {
                                    let comments = detail.comments.clone();
                                    view! {
                                        <div class="mb-4">
                                            {comments.into_iter().map(|c| {
                                                let author = c.author.clone();
                                                let body = c.body.clone();
                                                let time = c.created_at.clone();
                                                view! {
                                                    <div class="comment">
                                                        <div class="comment-header">
                                                            <span class="comment-author">{author}</span>
                                                            <span class="comment-time">{time}</span>
                                                        </div>
                                                        <p class="comment-body">{body}</p>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }
                                })}

                                // Add comment form
                                <div class="card">
                                    <div class="card-header">"Add a comment"</div>
                                    <ActionForm action=comment_action>
                                        <input type="hidden" name="owner" value={owner_name.clone()} />
                                        <input type="hidden" name="repo" value={repo_name.clone()} />
                                        <input type="hidden" name="number" value={num.to_string()} />
                                        <div class="form-group">
                                            <textarea
                                                name="body"
                                                required
                                                rows="3"
                                                class="form-textarea"
                                            ></textarea>
                                        </div>
                                        <button type="submit" class="btn btn-primary">"Comment"</button>
                                    </ActionForm>
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
