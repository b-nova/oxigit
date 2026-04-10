use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde::{Deserialize, Serialize};

use crate::components::copy_button::CopyButton;
use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[allow(unused_imports)]
use super::{AiMetadataInfo, CommitSummary, DiffReviewData, DiffSummaryInfo, RiskFlagInfo};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitDetail {
    pub commit: CommitSummary,
    pub diff_html: String,
    pub ai_metadata: Option<AiMetadataInfo>,
    pub preview_url: Option<String>,
    pub preview_status: Option<String>,
}

#[server]
async fn fetch_commit_diff(
    owner: String,
    repo: String,
    sha: String,
) -> Result<CommitDetail, ServerFnError> {
    use crate::server_fns::{
        extract_session_user, get_ai_access_level, get_repo_path, get_repo_pools, sfn_err,
    };
    use oxigit_core::{db, git};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user().await;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, current_user.as_ref().map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;
    let (commit_info, diff) = git::show_commit_diff(&repo_path, &sha).map_err(sfn_err)?;

    // Render diff as HTML with line coloring
    let diff_html = super::render_diff(&diff);

    // Fetch AI metadata — free users see tool/model badges, Flat+ sees full details
    use oxigit_core::entitlements::AiAccessLevel;
    let ai_access = match current_user.as_ref() {
        Some(u) => get_ai_access_level(u.id)
            .await
            .unwrap_or(AiAccessLevel::Limited),
        None => AiAccessLevel::Limited,
    };
    let ai_metadata = {
        db::get_ai_metadata_for_commit(&pool, repo_db.id, &sha)
            .await
            .ok()
            .flatten()
            .map(|m| {
                use super::AiMetadataInfo;
                let is_full = ai_access == AiAccessLevel::Full;
                AiMetadataInfo {
                    ai_tool: m.ai_tool,
                    ai_model: m.ai_model,
                    ai_prompt: if is_full { m.ai_prompt } else { None },
                    ai_session_id: if is_full { m.ai_session_id } else { None },
                    ai_files_touched: m
                        .ai_files_touched
                        .and_then(|f| serde_json::from_str(&f).ok()),
                    ai_prompt_index: m.ai_prompt_index,
                }
            })
    };

    // Check for deploy preview
    let preview = db::get_deploy_preview(&pool, repo_db.id, &sha)
        .await
        .ok()
        .flatten();

    Ok(CommitDetail {
        commit: CommitSummary {
            message: commit_info.message,
            author: commit_info.author,
            time: commit_info.time,
        },
        diff_html,
        ai_metadata,
        preview_url: preview.as_ref().and_then(|p| p.preview_url.clone()),
        preview_status: preview.map(|p| p.status),
    })
}

#[allow(clippy::too_many_arguments)]
#[server]
async fn attach_ai_metadata(
    owner: String,
    repo: String,
    commit_sha: String,
    ai_tool: String,
    ai_model: Option<String>,
    ai_prompt: Option<String>,
    ai_session_id: Option<String>,
    ai_files_touched: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::server_fns::{extract_session_user, get_repo_pools, sfn_err};
    use oxigit_core::db;

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    // Check write access
    let can_push = db::can_push_repo(&pool, &repo_db, user.id)
        .await
        .map_err(sfn_err)?;
    if !can_push {
        return Err(ServerFnError::new("Access denied"));
    }

    // Convert comma-separated files to JSON array if provided
    let files_json = ai_files_touched.map(|f| {
        let files: Vec<String> = f.split(',').map(|s| s.trim().to_string()).collect();
        serde_json::to_string(&files).unwrap_or_default()
    });

    db::insert_ai_metadata(
        &pool,
        repo_db.id,
        &commit_sha,
        &ai_tool,
        ai_model.as_deref(),
        ai_prompt.as_deref(),
        ai_session_id.as_deref(),
        files_json.as_deref(),
        None, // ai_prompt_index not available from manual annotation
    )
    .await
    .map_err(sfn_err)?;

    Ok(())
}

#[server]
async fn get_diff_review(
    owner: String,
    repo: String,
    sha: String,
) -> Result<DiffReviewData, ServerFnError> {
    use crate::server_fns::{
        extract_session_user, get_effective_llm_config, get_repo_path, get_repo_pools, sfn_err,
    };
    use oxigit_core::{db, git, llm, risk};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = extract_session_user().await;
    let (llm_provider, api_key, model, base_url) =
        get_effective_llm_config(current_user.as_ref().map(|u| u.id)).await?;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, current_user.map(|u| u.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;
    let (_, diff) = git::show_commit_diff(&repo_path, &sha).map_err(sfn_err)?;

    let risk_flags: Vec<RiskFlagInfo> = risk::scan_diff(&diff)
        .into_iter()
        .map(|f| RiskFlagInfo {
            category: f.category.label().to_string(),
            message: f.message,
            file: f.file,
        })
        .collect();

    // Check for cached summary
    let cached = db::get_diff_summary(&pool, repo_db.id, &sha)
        .await
        .ok()
        .flatten();

    let llm_available = llm_provider != "none";

    // Auto-generate summary if LLM is configured and no cache exists
    let summary = if let Some(s) = cached {
        let flags: Vec<RiskFlagInfo> = s
            .risk_flags
            .and_then(|f| serde_json::from_str(&f).ok())
            .unwrap_or_default();
        Some(DiffSummaryInfo {
            summary: s.summary,
            risk_flags: flags,
            generated_by: s.generated_by,
        })
    } else if llm_available {
        let ai_prompt = db::get_ai_metadata_for_commit(&pool, repo_db.id, &sha)
            .await
            .ok()
            .flatten()
            .and_then(|m| m.ai_prompt);
        let config = llm::LlmConfig {
            provider: llm_provider.clone(),
            api_key,
            model: model.clone(),
            base_url,
        };
        match llm::generate_summary(&config, &diff, ai_prompt.as_deref()).await {
            Ok(summary_text) => {
                let flags_json = serde_json::to_string(&risk_flags).ok();
                let _ = db::upsert_diff_summary(
                    &pool,
                    repo_db.id,
                    &sha,
                    &summary_text,
                    flags_json.as_deref(),
                    &model,
                )
                .await;
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
async fn generate_diff_summary(
    owner: String,
    repo: String,
    sha: String,
) -> Result<DiffSummaryInfo, ServerFnError> {
    use crate::server_fns::{
        extract_session_user, get_effective_llm_config, get_repo_path, get_repo_pools,
        get_user_entitlements, sfn_err,
    };
    use oxigit_core::{db, git, llm, risk};

    let user = extract_session_user()
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let entitlements = get_user_entitlements(user.id).await?;
    if !entitlements.ai_features {
        return Err(ServerFnError::new(
            "AI features require a Flat or higher plan. Upgrade at /pricing",
        ));
    }

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

    let repo_path = get_repo_path(&owner, &repo).await?;
    let (_, diff) = git::show_commit_diff(&repo_path, &sha).map_err(sfn_err)?;

    // Get AI prompt context if available
    let ai_prompt = db::get_ai_metadata_for_commit(&pool, repo_db.id, &sha)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.ai_prompt);

    let config = llm::LlmConfig {
        provider,
        api_key,
        model: model.clone(),
        base_url,
    };
    let summary = llm::generate_summary(&config, &diff, ai_prompt.as_deref())
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

    db::upsert_diff_summary(
        &pool,
        repo_db.id,
        &sha,
        &summary,
        flags_json.as_deref(),
        &model,
    )
    .await
    .map_err(sfn_err)?;

    Ok(DiffSummaryInfo {
        summary,
        risk_flags,
        generated_by: model,
    })
}

#[component]
pub fn CommitViewPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();
    let sha = move || params.read().get("sha").unwrap_or_default();

    let detail = Resource::new(
        move || (owner(), repo(), sha()),
        move |(owner, repo, sha)| fetch_commit_diff(owner, repo, sha),
    );

    let review = Resource::new(
        move || (owner(), repo(), sha()),
        move |(owner, repo, sha)| get_diff_review(owner, repo, sha),
    );

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                let commit_sha = sha();
                Suspend::new(async move {
                    match detail.await {
                        Ok(d) => {
                            let short_sha = &commit_sha[..7.min(commit_sha.len())];
                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <a href={format!("/{}/{}/commits", owner_name, repo_name)}>"commits"</a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span class="commit-sha">{short_sha.to_string()}</span>
                                        <CopyButton text=commit_sha.clone() />
                                    </h1>
                                </div>
                                <div class="card mb-4">
                                    <div class="commit-message" style="font-size: 1.125rem; margin-bottom: var(--space-2);">
                                        {d.commit.message.clone()}
                                    </div>
                                    <div class="commit-meta">
                                        {d.commit.author.clone()} " committed " {d.commit.time.clone()}
                                    </div>
                                    {d.preview_url.clone().map(|url| view! {
                                        <a href={url} target="_blank" rel="noopener" class="btn btn-sm btn-preview mt-2">
                                            "Live Preview"
                                        </a>
                                    })}
                                    {(d.preview_status.as_deref() == Some("pending")).then(|| view! {
                                        <span class="preview-pending mt-2">"Deploy in progress..."</span>
                                    })}
                                </div>
                                {d.ai_metadata.clone().map(|meta| {
                                    let session_href = meta.ai_session_id.clone().map(|sid| {
                                        format!("/{}/{}/ai/{}", owner_name, repo_name, sid)
                                    });
                                    view! {
                                        <div class="ai-context-panel mb-4">
                                            <div class="ai-context-header">
                                                <span class="ai-badge">{meta.ai_tool.clone()}</span>
                                                {meta.ai_model.clone().map(|model| view! {
                                                    <span class="ai-model-tag">{model}</span>
                                                })}
                                                {session_href.map(|href| view! {
                                                    <a href={href} class="ai-session-link">"View session"</a>
                                                })}
                                            </div>
                                            {meta.ai_prompt.clone().map(|prompt| view! {
                                                <details class="ai-prompt-details">
                                                    <summary>"Prompt"</summary>
                                                    <div class="ai-prompt-bubble">{prompt}</div>
                                                </details>
                                            })}
                                            {meta.ai_files_touched.clone().map(|files| view! {
                                                <div class="ai-files-touched">
                                                    <span class="text-secondary">"AI-touched files: "</span>
                                                    {files.join(", ")}
                                                </div>
                                            })}
                                        </div>
                                    }
                                })}
                                // AI Diff Review panel
                                <Suspense fallback=|| view! { <p class="text-secondary" style="font-size: 0.8125rem;">"Analyzing diff..."</p> }>
                                    {move || {
                                        Suspend::new(async move {
                                            let review_data = review.await.ok();
                                            let has_risk = review_data.as_ref().is_some_and(|r| !r.risk_flags.is_empty());
                                            let has_summary = review_data.as_ref().is_some_and(|r| r.cached_summary.is_some());

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
                                <div class="diff-container" inner_html={d.diff_html}></div>
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
