use leptos::prelude::*;

use crate::components::icons::{IconBranch, IconPlus, IconRepo, IconRust, IconSearch, IconServer};

#[allow(unused_imports)]
use super::{get_current_user, DashboardData, RecentSessionInfo, RiskCountInfo, ToolUsageInfo};

#[server]
async fn fetch_dashboard() -> Result<Option<DashboardData>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool};
    use oxigit_core::db;
    use std::collections::HashMap;

    let user = match extract_session_user().await {
        Some(u) => u,
        None => return Ok(None),
    };
    let pool = get_pool().await?;

    let stats = db::get_user_ai_stats(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let tool_usage = db::get_user_tool_usage(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .into_iter()
        .map(|t| ToolUsageInfo { ai_tool: t.ai_tool, commit_count: t.commit_count })
        .collect();

    let recent_sessions = db::get_user_recent_sessions(&pool, user.id, 5)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .into_iter()
        .map(|s| RecentSessionInfo {
            session_id: s.session_id,
            ai_tool: s.ai_tool,
            repo_name: s.repo_name,
            commit_count: s.commit_count,
            last_time: s.last_time,
            first_prompt: s.first_prompt,
        })
        .collect();

    // Aggregate risk flags from summaries
    let risk_rows = db::get_user_risk_summaries(&pool, user.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut risk_map: HashMap<String, i64> = HashMap::new();
    for row in &risk_rows {
        if let Some(ref flags_json) = row.risk_flags {
            if let Ok(flags) = serde_json::from_str::<Vec<super::RiskFlagInfo>>(flags_json) {
                for flag in flags {
                    *risk_map.entry(flag.category).or_insert(0) += 1;
                }
            }
        }
    }
    let risk_counts: Vec<RiskCountInfo> = risk_map
        .into_iter()
        .map(|(category, count)| RiskCountInfo { category, count })
        .collect();

    Ok(Some(DashboardData {
        total_ai_commits: stats.total_ai_commits,
        total_sessions: stats.total_sessions,
        total_repos_with_ai: stats.total_repos_with_ai,
        tool_usage,
        recent_sessions,
        risk_counts,
    }))
}

#[component]
pub fn HomePage() -> impl IntoView {
    let user = Resource::new(|| (), |_| get_current_user());
    let dashboard = Resource::new(|| (), |_| fetch_dashboard());

    view! {
        <Suspense fallback=|| view! {
            <div class="hero">
                <h1 class="hero-title">"Oxigit"</h1>
            </div>
        }>
            {move || Suspend::new(async move {
                match user.await {
                    Ok(Some(u)) => {
                        let username = u.username.clone();
                        let dash = dashboard.await.ok().flatten();

                        // Build dashboard sections as a Vec to flatten type depth
                        let mut sections: Vec<AnyView> = Vec::new();

                        sections.push(view! {
                            <h1 class="dashboard-greeting">"Welcome back, " {username.clone()}</h1>
                        }.into_any());

                        if let Some(ref d) = dash {
                            let top_tool = d.tool_usage.first().map(|t| t.ai_tool.clone()).unwrap_or_else(|| "-".into());
                            sections.push(view! {
                                <div class="dashboard-stats mb-4">
                                    <div class="stat-card">
                                        <div class="stat-number">{d.total_ai_commits}</div>
                                        <div class="stat-label">"AI Commits"</div>
                                    </div>
                                    <div class="stat-card">
                                        <div class="stat-number">{d.total_sessions}</div>
                                        <div class="stat-label">"Sessions"</div>
                                    </div>
                                    <div class="stat-card">
                                        <div class="stat-number">{d.total_repos_with_ai}</div>
                                        <div class="stat-label">"AI Repos"</div>
                                    </div>
                                    <div class="stat-card">
                                        <div class="stat-number stat-text">{top_tool}</div>
                                        <div class="stat-label">"Top Tool"</div>
                                    </div>
                                </div>
                            }.into_any());

                            if !d.tool_usage.is_empty() {
                                let max_count = d.tool_usage.first().map(|t| t.commit_count).unwrap_or(1);
                                let bars: Vec<AnyView> = d.tool_usage.iter().map(|t| {
                                    let pct = (t.commit_count as f64 / max_count as f64 * 100.0) as i64;
                                    view! {
                                        <div class="tool-usage-row">
                                            <div class="tool-usage-label">
                                                <span class="ai-badge">{t.ai_tool.clone()}</span>
                                                <span class="text-secondary">{t.commit_count} " commits"</span>
                                            </div>
                                            <div class="tool-usage-bar-bg">
                                                <div class="tool-usage-bar-fill" style={format!("width: {}%", pct)}></div>
                                            </div>
                                        </div>
                                    }.into_any()
                                }).collect();
                                sections.push(view! {
                                    <div class="dashboard-section mb-4">
                                        <div class="dashboard-section-title">"AI Tool Usage"</div>
                                        {bars}
                                    </div>
                                }.into_any());
                            }

                            if !d.risk_counts.is_empty() {
                                let risks: Vec<AnyView> = d.risk_counts.iter().map(|r| {
                                    let class = format!("risk-badge risk-{}", r.category);
                                    view! {
                                        <div class="risk-summary-row">
                                            <span class={class}>{r.category.clone()}</span>
                                            <span class="text-secondary">{r.count} " flags"</span>
                                        </div>
                                    }.into_any()
                                }).collect();
                                sections.push(view! {
                                    <div class="dashboard-section mb-4">
                                        <div class="dashboard-section-title">"Risk Overview"</div>
                                        {risks}
                                    </div>
                                }.into_any());
                            }

                            if !d.recent_sessions.is_empty() {
                                let items: Vec<AnyView> = d.recent_sessions.iter().map(|s| {
                                    let prompt = s.first_prompt.clone()
                                        .map(|p| if p.len() > 80 { format!("{}...", &p[..80]) } else { p })
                                        .unwrap_or_default();
                                    let href = format!("/{}/{}/ai/{}", username, s.repo_name, s.session_id);
                                    view! {
                                        <li class="list-item">
                                            <div>
                                                <div class="flex-row gap-2">
                                                    <span class="ai-badge">{s.ai_tool.clone()}</span>
                                                    <a href={href} class="list-item-title">{s.repo_name.clone()}</a>
                                                    <span class="text-tertiary">{s.commit_count} " commits"</span>
                                                </div>
                                                {(!prompt.is_empty()).then(|| view! {
                                                    <p class="list-item-desc">{prompt}</p>
                                                })}
                                            </div>
                                            <span class="list-item-meta">{s.last_time.clone()}</span>
                                        </li>
                                    }.into_any()
                                }).collect();
                                sections.push(view! {
                                    <div class="card mb-4">
                                        <div class="card-header">"Recent AI Sessions"</div>
                                        <ul class="list">{items}</ul>
                                    </div>
                                }.into_any());
                            }
                        }

                        sections.push(view! {
                            <div class="dashboard-grid">
                                <a href="/repos" class="card-link">
                                    <div class="feature-card-icon"><IconRepo /></div>
                                    <div class="card-header">"Your Repositories"</div>
                                    <p class="text-secondary" style="font-size: 0.875rem;">"View and manage your repos"</p>
                                </a>
                                <a href="/repos/new" class="card-link">
                                    <div class="feature-card-icon"><IconPlus /></div>
                                    <div class="card-header">"New Repository"</div>
                                    <p class="text-secondary" style="font-size: 0.875rem;">"Create a new project"</p>
                                </a>
                                <a href="/explore" class="card-link">
                                    <div class="feature-card-icon"><IconSearch /></div>
                                    <div class="card-header">"Explore"</div>
                                    <p class="text-secondary" style="font-size: 0.875rem;">"Discover public repositories"</p>
                                </a>
                            </div>
                        }.into_any());

                        view! {
                            <div class="animate-in">{sections}</div>
                        }.into_any()
                    }
                    _ => view! {
                        <div class="hero animate-in">
                            <h1 class="hero-title">"Oxigit"</h1>
                            <p class="hero-subtitle">
                                "The AI-native Git platform for vibecoders. Track what your AI builds, review it, remix it, deploy it."
                            </p>
                            <div class="hero-actions">
                                <a href="/login" class="btn btn-lg btn-outline">"Sign in"</a>
                                <a href="/register" class="btn btn-lg btn-primary">"Get started"</a>
                            </div>
                            <div class="hero-features">
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconBranch /></div>
                                    <div class="feature-card-title">"AI-Aware Commits"</div>
                                    <div class="feature-card-desc">"Every commit tracks which AI tool and prompt generated it. Browse your repo as a conversation timeline."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconSearch /></div>
                                    <div class="feature-card-title">"Smart Diff Review"</div>
                                    <div class="feature-card-desc">"Auto-generated summaries and risk detection on every diff. Understand what your AI wrote, instantly."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconRepo /></div>
                                    <div class="feature-card-title">"Session Snapshots"</div>
                                    <div class="feature-card-desc">"Browse, review, and revert entire AI coding sessions. The session is the new unit of work."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconPlus /></div>
                                    <div class="feature-card-title">"Remix Projects"</div>
                                    <div class="feature-card-desc">"One-click remix with starter prompts. Fork a project and start vibecoding on it immediately."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconServer /></div>
                                    <div class="feature-card-title">"Deploy Previews"</div>
                                    <div class="feature-card-desc">"Webhook-based live previews for every push. See if it works before reading a single line of code."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconRust /></div>
                                    <div class="feature-card-title">"Self-Hosted Rust"</div>
                                    <div class="feature-card-desc">"Your code, your server. Built with Rust for blazing speed and memory safety. Git over HTTP and SSH."</div>
                                </div>
                            </div>
                        </div>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}
