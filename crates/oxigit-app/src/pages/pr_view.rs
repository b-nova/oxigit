use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;
use crate::components::toast::use_toast;

#[allow(unused_imports)]
use super::{CommitSummary, DiffReviewData, DiffSummaryInfo, RiskFlagInfo};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrDetail {
    pub number: i64,
    pub title: String,
    pub description: String,
    pub author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub status: String,
    pub merged_by: Option<String>,
    pub created_at: String,
    pub commits: Vec<CommitSummary>,
    pub diff_html: String,
    pub can_merge: bool,
    pub is_owner_or_author: bool,
}

#[server]
async fn get_pr(
    owner: String,
    repo: String,
    number: i64,
) -> Result<PrDetail, ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_path, get_repo_pools};
    use oxigit_core::{db, git};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, current_user.as_ref().map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let pr = db::get_pull_request(&pool, repo_db.id, number)
        .await
        .map_err(sfn_err)?;

    let author = db::get_user_by_id(&control_pool, pr.author_id)
        .await
        .map_err(sfn_err)?;

    let merged_by = if let Some(uid) = pr.merged_by {
        Some(db::get_user_by_id(&control_pool, uid).await.map(|u| u.username).unwrap_or_default())
    } else {
        None
    };

    let repo_path = get_repo_path(&owner, &repo).await?;

    // Get commits and diff (only for open PRs with existing branches)
    let (commits, diff_html, mergeable) = if pr.status == "open"
        && git::branch_exists(&repo_path, &pr.source_branch)
        && git::branch_exists(&repo_path, &pr.target_branch)
    {
        let commits = git::branch_commits(&repo_path, &pr.target_branch, &pr.source_branch)
            .unwrap_or_default()
            .into_iter()
            .map(|c| CommitSummary {
                message: c.message,
                author: c.author,
                time: c.time,
            })
            .collect();

        let diff = git::branch_diff(&repo_path, &pr.target_branch, &pr.source_branch)
            .unwrap_or_default();
        let diff_html = super::render_diff(&diff);

        let mergeable = git::can_merge(&repo_path, &pr.target_branch, &pr.source_branch)
            .unwrap_or(false);

        (commits, diff_html, mergeable)
    } else {
        (vec![], String::new(), false)
    };

    let viewer_id = current_user.map(|u| u.id);
    let is_owner_or_author = viewer_id
        .map(|id| id == repo_db.owner_id || id == pr.author_id)
        .unwrap_or(false);

    Ok(PrDetail {
        number: pr.number,
        title: pr.title,
        description: pr.description,
        author: author.username,
        source_branch: pr.source_branch,
        target_branch: pr.target_branch,
        status: pr.status,
        merged_by,
        created_at: pr.created_at,
        commits,
        diff_html,
        can_merge: mergeable && is_owner_or_author,
        is_owner_or_author,
    })
}

#[server]
async fn merge_pr(
    owner: String,
    repo: String,
    number: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_path, get_repo_pools};
    use oxigit_core::{db, git};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let pr = db::get_pull_request(&pool, repo_db.id, number)
        .await
        .map_err(sfn_err)?;

    // Only owner or PR author can merge
    if user.id != repo_db.owner_id && user.id != pr.author_id {
        return Err(ServerFnError::new("Not authorized to merge"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;
    let message = format!("Merge pull request #{} from {}\n\n{}", pr.number, pr.source_branch, pr.title);

    git::merge_branches(&repo_path, &pr.target_branch, &pr.source_branch, &message)
        .map_err(sfn_err)?;

    db::merge_pull_request(&pool, repo_db.id, number, user.id)
        .await
        .map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/pulls/{}", owner, repo, number));
    Ok(())
}

#[server]
async fn close_pr(
    owner: String,
    repo: String,
    number: i64,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_pools};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    let pr = db::get_pull_request(&pool, repo_db.id, number)
        .await
        .map_err(sfn_err)?;

    if user.id != repo_db.owner_id && user.id != pr.author_id {
        return Err(ServerFnError::new("Not authorized"));
    }

    db::close_pull_request(&pool, repo_db.id, number)
        .await
        .map_err(sfn_err)?;

    leptos_axum::redirect(&format!("/{}/{}/pulls/{}", owner, repo, number));
    Ok(())
}

#[server]
async fn get_pr_diff_review(
    owner: String,
    repo: String,
    number: i64,
) -> Result<DiffReviewData, ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_path, get_repo_pools, get_effective_llm_config};
    use oxigit_core::{db, git, llm, risk};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user().await;
    let (llm_provider, api_key, model, base_url) = get_effective_llm_config(current_user.as_ref().map(|u| u.id)).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let pr = db::get_pull_request(&pool, repo_db.id, number)
        .await
        .map_err(sfn_err)?;

    let repo_path = get_repo_path(&owner, &repo).await?;
    let diff = if pr.status == "open"
        && git::branch_exists(&repo_path, &pr.source_branch)
        && git::branch_exists(&repo_path, &pr.target_branch)
    {
        git::branch_diff(&repo_path, &pr.target_branch, &pr.source_branch).unwrap_or_default()
    } else {
        String::new()
    };

    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
        .into_iter()
        .map(|f| RiskFlagInfo {
            category: f.category.label().to_string(),
            message: f.message,
            file: f.file,
        })
        .collect();

    let cache_key = format!("pr-{}-diff", number);
    let cached = db::get_diff_summary(&pool, repo_db.id, &cache_key)
        .await
        .ok()
        .flatten();

    let llm_available = llm_provider != "none";

    // Auto-generate summary if LLM is configured and no cache exists
    let summary = if let Some(s) = cached {
        let flags: Vec<RiskFlagInfo> = s.risk_flags
            .and_then(|f| serde_json::from_str(&f).ok())
            .unwrap_or_default();
        Some(DiffSummaryInfo {
            summary: s.summary,
            risk_flags: flags,
            generated_by: s.generated_by,
        })
    } else if llm_available && !diff.is_empty() {
        let config = llm::LlmConfig { provider: llm_provider.clone(), api_key, model: model.clone(), base_url };
        match llm::generate_summary(&config, &diff, None).await {
            Ok(summary_text) => {
                let flags_json = serde_json::to_string(&risk_flags).ok();
                let _ = db::upsert_diff_summary(&pool, repo_db.id, &cache_key, &summary_text, flags_json.as_deref(), &model).await;
                Some(DiffSummaryInfo {
                    summary: summary_text,
                    risk_flags: risk_flags.clone(),
                    generated_by: model,
                })
            }
            Err(_) => None,
        }
    } else {
        None
    };

    Ok(DiffReviewData {
        cached_summary: summary,
        risk_flags,
        llm_available,
    })
}

#[server]
async fn generate_pr_diff_summary(
    owner: String,
    repo: String,
    number: i64,
) -> Result<DiffSummaryInfo, ServerFnError> {
    use crate::server_fns::{sfn_err, extract_session_user, get_repo_path, get_repo_pools, get_effective_llm_config};
    use oxigit_core::{db, git, llm, risk};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let (provider, api_key, model, base_url) = get_effective_llm_config(Some(user.id)).await?;

    if provider == "none" {
        return Err(ServerFnError::new("LLM not configured"));
    }

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, Some(user.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let pr = db::get_pull_request(&pool, repo_db.id, number)
        .await
        .map_err(sfn_err)?;

    let repo_path = get_repo_path(&owner, &repo).await?;
    let diff = git::branch_diff(&repo_path, &pr.target_branch, &pr.source_branch)
        .map_err(sfn_err)?;

    let config = llm::LlmConfig { provider, api_key, model: model.clone(), base_url };
    let summary = llm::generate_summary(&config, &diff, None)
        .await
        .map_err(sfn_err)?;

    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
        .into_iter()
        .map(|f| RiskFlagInfo {
            category: f.category.label().to_string(),
            message: f.message,
            file: f.file,
        })
        .collect();

    let flags_json = serde_json::to_string(&risk_flags).ok();
    let cache_key = format!("pr-{}-diff", number);

    db::upsert_diff_summary(&pool, repo_db.id, &cache_key, &summary, flags_json.as_deref(), &model)
        .await
        .map_err(sfn_err)?;

    Ok(DiffSummaryInfo {
        summary,
        risk_flags,
        generated_by: model,
    })
}

#[component]
pub fn PrViewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let number = move || params.read().get("number").and_then(|n| n.parse::<i64>().ok()).unwrap_or(0);

    let pr = Resource::new(
        move || (owner(), repo(), number()),
        move |(o, r, n)| get_pr(o, r, n),
    );

    let pr_review = Resource::new(
        move || (owner(), repo(), number()),
        move |(o, r, n)| get_pr_diff_review(o, r, n),
    );

    let merge_action = ServerAction::<MergePr>::new();
    let close_action = ServerAction::<ClosePr>::new();

    let toast = use_toast();
    let toast_merge = toast.clone();
    let toast_close = toast.clone();
    Effect::new(move |_| {
        if let Some(Ok(_)) = merge_action.value().get() {
            toast_merge.success("Pull request merged");
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(_)) = close_action.value().get() {
            toast_close.success("Pull request closed");
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match pr.await {
                        Ok(detail) => {
                            let badge_class = match detail.status.as_str() {
                                "open" => "badge badge-open",
                                "merged" => "badge badge-merged",
                                "closed" => "badge badge-closed",
                                _ => "badge",
                            };
                            let num = detail.number;

                            view! {
                                <div class="page-header">
                                    <h1 class="page-title-sm">
                                        {detail.title.clone()}
                                        <span class="text-secondary" style="font-weight: 400;"> " #" {num}</span>
                                    </h1>
                                </div>

                                <div class="flex-row gap-3 mb-4" style="flex-wrap: wrap;">
                                    <span class={badge_class}>{detail.status.clone()}</span>
                                    <span class="text-secondary" style="font-size: 0.875rem;">
                                        {detail.author.clone()} " wants to merge "
                                        <span class="badge-branch">{detail.source_branch.clone()}</span>
                                        " into "
                                        <span class="badge-branch">{detail.target_branch.clone()}</span>
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

                                // Merge/Close buttons
                                {(detail.status == "open" && detail.is_owner_or_author).then(|| {
                                    let on = owner_name.clone();
                                    let rn = repo_name.clone();
                                    view! {
                                        <div class="card mb-4 flex-row gap-3">
                                            {detail.can_merge.then(|| {
                                                let o = on.clone();
                                                let r = rn.clone();
                                                view! {
                                                    <ActionForm action=merge_action>
                                                        <input type="hidden" name="owner" value={o} />
                                                        <input type="hidden" name="repo" value={r} />
                                                        <input type="hidden" name="number" value={num.to_string()} />
                                                        <button type="submit" class="btn btn-primary">"Merge Pull Request"</button>
                                                    </ActionForm>
                                                }
                                            })}
                                            {(!detail.can_merge).then(|| view! {
                                                <span class="text-secondary" style="font-size: 0.875rem;">"This branch cannot be fast-forward merged."</span>
                                            })}
                                            <ActionForm action=close_action>
                                                <input type="hidden" name="owner" value={on.clone()} />
                                                <input type="hidden" name="repo" value={rn.clone()} />
                                                <input type="hidden" name="number" value={num.to_string()} />
                                                <button type="submit" class="btn btn-danger">"Close"</button>
                                            </ActionForm>
                                        </div>
                                    }
                                })}

                                // Merged info
                                {detail.merged_by.clone().map(|by| view! {
                                    <div class="card mb-4">
                                        <span class="text-merged">"Merged"</span> " by " <strong>{by}</strong>
                                    </div>
                                })}

                                // Commits
                                {(!detail.commits.is_empty()).then(|| {
                                    let commits = detail.commits.clone();
                                    view! {
                                        <div class="card-flush mb-4">
                                            <div class="card-header">{commits.len()} " commit(s)"</div>
                                            <ul class="list">
                                                {commits.into_iter().map(|c| {
                                                    let msg = c.message.clone();
                                                    let author = c.author.clone();
                                                    view! {
                                                        <li class="commit-item">
                                                            <span class="commit-message">{msg}</span>
                                                            <span class="commit-meta">{author}</span>
                                                        </li>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </ul>
                                        </div>
                                    }
                                })}

                                // AI Diff Review panel
                                <Suspense fallback=|| view! { <p class="text-secondary" style="font-size: 0.8125rem;">"Analyzing diff..."</p> }>
                                    {move || {
                                        Suspend::new(async move {
                                            let review_data = pr_review.await.ok();
                                            let has_risk = review_data.as_ref().map_or(false, |r| !r.risk_flags.is_empty());
                                            let has_summary = review_data.as_ref().map_or(false, |r| r.cached_summary.is_some());

                                            if !has_risk && !has_summary {
                                                return view! { <div></div> }.into_any();
                                            }

                                            let risk_flags = review_data.as_ref().map(|r| r.risk_flags.clone()).unwrap_or_default();
                                            let cached = review_data.and_then(|r| r.cached_summary);

                                            view! {
                                                <div class="ai-review-panel mb-4">
                                                    <div class="ai-review-header">
                                                        <span class="ai-review-title">"AI Diff Review"</span>
                                                    </div>
                                                    {(!risk_flags.is_empty()).then(|| {
                                                        let flags = risk_flags.clone();
                                                        view! {
                                                            <div class="risk-badges">
                                                                {flags.into_iter().map(|f| {
                                                                    let class = format!("risk-badge risk-{}", f.category);
                                                                    let label = format!("{}: {}", f.category, f.message);
                                                                    view! {
                                                                        <span class={class} title={f.file.unwrap_or_default()}>
                                                                            {label}
                                                                        </span>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </div>
                                                        }
                                                    })}
                                                    {cached.map(|s| view! {
                                                        <div class="ai-review-summary">{s.summary}</div>
                                                        <div class="ai-review-meta">
                                                            "Generated by " {s.generated_by}
                                                        </div>
                                                    })}
                                                </div>
                                            }.into_any()
                                        })
                                    }}
                                </Suspense>

                                // Diff
                                {(!detail.diff_html.is_empty()).then(|| {
                                    let dh = detail.diff_html.clone();
                                    view! {
                                        <div class="diff-container" inner_html={dh}></div>
                                    }
                                })}
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
