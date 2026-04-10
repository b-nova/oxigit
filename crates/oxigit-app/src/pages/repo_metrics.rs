use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingPage;

#[allow(unused_imports)]
use super::{RepoMetricsResponse, RiskCountInfo, SessionScoreItem, ToolScoreInfo, VibeScoreInfo};

#[server]
async fn fetch_repo_metrics(
    owner: String,
    repo: String,
) -> Result<RepoMetricsResponse, ServerFnError> {
    use crate::server_fns::{
        get_ai_access_level, get_repo_path, get_repo_pools, require_auth, sfn_err,
    };
    use oxigit_core::{db, entitlements::AiAccessLevel, git, risk, vibe};

    let (control_pool, pool) = get_repo_pools(&owner, &repo).await?;
    let current_user = require_auth().await?;

    let ai_access = get_ai_access_level(current_user.id).await?;
    let is_limited = ai_access == AiAccessLevel::Limited;

    let (_, repo_db) = db::get_repository_cross(&control_pool, &pool, &owner, &repo)
        .await
        .map_err(sfn_err)?;

    if !db::can_access_repo(&repo_db, Some(current_user.id)) {
        return Err(ServerFnError::new("Repository not found"));
    }

    let repo_path = get_repo_path(&owner, &repo).await?;

    let session_data = db::get_repo_session_data(&pool, repo_db.id)
        .await
        .map_err(sfn_err)?;

    // Free plan: limit to last 30 days
    let session_data = if is_limited {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();
        session_data
            .into_iter()
            .filter(|sd| sd.last_time.as_str() >= cutoff.as_str())
            .collect()
    } else {
        session_data
    };

    let mut session_scores = Vec::new();
    let mut tool_scores: std::collections::HashMap<String, (i64, u64)> =
        std::collections::HashMap::new();
    let mut total_prompts = 0i64;
    let mut all_risk_flags: Vec<super::RiskFlagInfo> = Vec::new();

    for sd in &session_data {
        let shas = sd.commit_shas.clone();

        // Get diff stats
        let (lines_added, lines_deleted) =
            git::session_diff_stats(&repo_path, &shas).unwrap_or((0, 0));

        // Get risk flags from cached summary or scan
        let cache_key = format!("session-{}", sd.session_id);
        let risk_count =
            if let Ok(Some(summary)) = db::get_diff_summary(&pool, repo_db.id, &cache_key).await {
                if let Some(flags_json) = &summary.risk_flags {
                    let flags: Vec<super::RiskFlagInfo> =
                        serde_json::from_str(flags_json).unwrap_or_default();
                    let count = flags.len();
                    all_risk_flags.extend(flags);
                    count
                } else {
                    0
                }
            } else {
                // Compute from diff if no cached summary
                let diff = git::session_aggregate_diff(&repo_path, &shas).unwrap_or_default();
                if !diff.is_empty() {
                    let flags = risk::scan_diff(&diff);
                    let count = flags.len();
                    for f in flags {
                        all_risk_flags.push(super::RiskFlagInfo {
                            category: f.category.label().to_string(),
                            message: f.message,
                            file: f.file,
                        });
                    }
                    count
                } else {
                    0
                }
            };

        // Get file count from commit metadata
        let files_touched = {
            let metas = db::get_ai_metadata_by_session(&pool, repo_db.id, &sd.session_id)
                .await
                .unwrap_or_default();
            let mut files: Vec<String> = Vec::new();
            for meta in &metas {
                if let Some(ref fj) = meta.ai_files_touched
                    && let Ok(fs) = serde_json::from_str::<Vec<String>>(fj)
                {
                    for f in fs {
                        if !files.contains(&f) {
                            files.push(f);
                        }
                    }
                }
            }
            files.len()
        };

        let was_reverted = git::is_session_reverted(&repo_path, &sd.session_id);

        let metrics = vibe::SessionMetrics {
            commit_count: sd.commit_count as usize,
            prompt_count: sd.prompt_count as usize,
            risk_flag_count: risk_count,
            files_touched,
            lines_added,
            lines_deleted,
            was_reverted,
        };
        let vs = vibe::compute_vibe_score(&metrics);

        // Accumulate tool scores
        let entry = tool_scores.entry(sd.ai_tool.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += vs.score as u64;

        total_prompts += sd.prompt_count;

        session_scores.push(SessionScoreItem {
            session_id: sd.session_id.clone(),
            ai_tool: sd.ai_tool.clone(),
            score: vs.score,
            grade: vs.grade.to_string(),
            commit_count: sd.commit_count,
            prompt_count: sd.prompt_count,
            first_time: sd.first_time.clone(),
            last_time: sd.last_time.clone(),
            first_prompt: sd.first_prompt.clone(),
        });
    }

    let average_score = if session_scores.is_empty() {
        0.0
    } else {
        session_scores.iter().map(|s| s.score as f64).sum::<f64>() / session_scores.len() as f64
    };

    let tool_comparison: Vec<ToolScoreInfo> = tool_scores
        .into_iter()
        .map(|(tool, (count, total))| ToolScoreInfo {
            ai_tool: tool,
            session_count: count,
            average_score: total as f64 / count.max(1) as f64,
        })
        .collect();

    // Aggregate risk distribution
    let mut risk_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for flag in &all_risk_flags {
        *risk_map.entry(flag.category.clone()).or_insert(0) += 1;
    }
    let risk_distribution: Vec<RiskCountInfo> = risk_map
        .into_iter()
        .map(|(category, count)| RiskCountInfo { category, count })
        .collect();

    Ok(RepoMetricsResponse {
        owner,
        repo,
        average_score,
        total_sessions: session_scores.len() as i64,
        total_prompts,
        session_scores,
        tool_comparison,
        risk_distribution,
        data_range_days: if is_limited { Some(30) } else { None },
        is_limited,
    })
}

#[component]
pub fn RepoMetricsPage() -> impl IntoView {
    let params = use_params_map();
    let owner = move || params.read().get("owner").unwrap_or_default();
    let repo = move || params.read().get("repo").unwrap_or_default();

    let data = Resource::new(
        move || (owner(), repo()),
        move |(o, r)| fetch_repo_metrics(o, r),
    );

    view! {
        <Suspense fallback=|| view! { <LoadingPage /> }>
            {move || {
                let owner_name = owner();
                let repo_name = repo();
                Suspend::new(async move {
                    match data.await {
                        Ok(d) => {
                            let limited = d.is_limited;
                            let range_days = d.data_range_days;
                            let avg_score = d.average_score.round() as u8;
                            let avg_grade = match avg_score {
                                80..=100 => 'A', 60..=79 => 'B', 40..=59 => 'C', 20..=39 => 'D', _ => 'F',
                            };
                            let grade_class = format!("vibe-{}", avg_grade);

                            // Session score rows
                            let session_rows: Vec<AnyView> = d.session_scores.iter().map(|s| {
                                let short_id = s.session_id[..8.min(s.session_id.len())].to_string();
                                let grade_class = format!("vibe-{}", s.grade);
                                let prompt_preview = s.first_prompt.clone()
                                    .map(|p| if p.len() > 60 { format!("{}...", &p[..60]) } else { p })
                                    .unwrap_or_default();
                                let session_cell: AnyView = if limited {
                                    view! { <td><span class="text-secondary">{short_id}</span></td> }.into_any()
                                } else {
                                    let href = format!("/{}/{}/ai/{}", owner_name, repo_name, s.session_id);
                                    view! { <td><a href={href} class="commit-sha">{short_id}</a></td> }.into_any()
                                };
                                view! {
                                    <tr>
                                        <td>
                                            <span class={format!("vibe-badge {}", grade_class)}>
                                                {s.score.to_string()} " " {s.grade.clone()}
                                            </span>
                                        </td>
                                        {session_cell}
                                        <td><span class="ai-badge">{s.ai_tool.clone()}</span></td>
                                        <td class="text-secondary">{prompt_preview}</td>
                                        <td class="text-tertiary">{s.commit_count} "c / " {s.prompt_count} "p"</td>
                                        <td class="text-tertiary">{s.last_time.clone()}</td>
                                    </tr>
                                }.into_any()
                            }).collect();

                            // Tool comparison
                            let max_tool_score = d.tool_comparison.iter()
                                .map(|t| t.average_score).fold(0.0f64, f64::max).max(1.0);
                            let tool_bars: Vec<AnyView> = d.tool_comparison.iter().map(|t| {
                                let width_pct = (t.average_score / max_tool_score * 100.0) as u32;
                                let tool_grade = match t.average_score.round() as u8 {
                                    80..=100 => 'A', 60..=79 => 'B', 40..=59 => 'C', 20..=39 => 'D', _ => 'F',
                                };
                                view! {
                                    <div class="tool-usage-row">
                                        <span class="tool-usage-name">{t.ai_tool.clone()}</span>
                                        <span class={format!("vibe-badge vibe-{}", tool_grade)}>
                                            {format!("{:.0}", t.average_score)} " " {tool_grade.to_string()}
                                        </span>
                                        <span class="text-tertiary">{t.session_count} " sessions"</span>
                                        <div class="tool-usage-bar-bg">
                                            <div class="tool-usage-bar-fill" style={format!("width: {}%", width_pct)}></div>
                                        </div>
                                    </div>
                                }.into_any()
                            }).collect();

                            // Risk distribution
                            let risk_badges: Vec<AnyView> = d.risk_distribution.iter().map(|r| {
                                let badge_class = format!("risk-badge risk-{}", r.category.to_lowercase());
                                view! {
                                    <span class={badge_class}>{r.category.clone()} " " {r.count.to_string()}</span>
                                }.into_any()
                            }).collect();

                            view! {
                                <div class="page-header">
                                    <h1 class="breadcrumb">
                                        <a href={format!("/{}/{}", owner_name, repo_name)}>
                                            {owner_name.clone()} <span class="breadcrumb-sep">" / "</span> {repo_name.clone()}
                                        </a>
                                        <span class="breadcrumb-sep">" / "</span>
                                        <span>"Metrics"</span>
                                    </h1>
                                </div>

                                {range_days.map(|days| view! {
                                    <div class="flash flash-info mb-4">
                                        {format!("Showing last {} days. ", days)}
                                        <a href="/pricing">"Upgrade to Flat"</a>
                                        " for full history."
                                    </div>
                                })}

                                // Stat cards
                                <div class="dashboard-stats mb-4">
                                    <div class="stat-card">
                                        <div class={format!("vibe-score-large {}", grade_class)}>
                                            {avg_score.to_string()}
                                        </div>
                                        <div class="stat-label">"Avg Vibe Score (" {avg_grade.to_string()} ")"</div>
                                    </div>
                                    <div class="stat-card">
                                        <div class="stat-number">{d.total_sessions.to_string()}</div>
                                        <div class="stat-label">"Sessions"</div>
                                    </div>
                                    <div class="stat-card">
                                        <div class="stat-number">{d.total_prompts.to_string()}</div>
                                        <div class="stat-label">"Prompts"</div>
                                    </div>
                                    <div class="stat-card">
                                        <div class="stat-number">{d.session_scores.len().to_string()}</div>
                                        <div class="stat-label">"Scored"</div>
                                    </div>
                                </div>

                                // Tool comparison
                                {(!d.tool_comparison.is_empty()).then(|| view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Tool Comparison"</div>
                                        <div style="padding: var(--space-4);">{tool_bars}</div>
                                    </div>
                                })}

                                // Risk distribution
                                {(!risk_badges.is_empty()).then(|| view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Risk Distribution"</div>
                                        <div style="padding: var(--space-4); display: flex; gap: var(--space-2); flex-wrap: wrap;">
                                            {risk_badges}
                                        </div>
                                    </div>
                                })}

                                // Session scores table
                                <div class="card-flush mb-4">
                                    <div class="card-header">"Session Scores"</div>
                                    <table class="session-score-table">
                                        <thead>
                                            <tr>
                                                <th>"Score"</th>
                                                <th>"Session"</th>
                                                <th>"Tool"</th>
                                                <th>"Prompt"</th>
                                                <th>"Size"</th>
                                                <th>"Last Active"</th>
                                            </tr>
                                        </thead>
                                        <tbody>{session_rows}</tbody>
                                    </table>
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
