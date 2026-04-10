use leptos::prelude::*;

use crate::components::icons::{IconBranch, IconPlus, IconRepo, IconRust, IconSearch, IconServer};

#[allow(unused_imports)]
use super::{DashboardData, RecentSessionInfo, RiskCountInfo, ToolUsageInfo, get_current_user};

#[server]
async fn fetch_dashboard() -> Result<Option<DashboardData>, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_control_pool, sfn_err};
    use oxigit_core::{db, vibe};
    use std::collections::HashMap;

    let user = match extract_session_user().await {
        Some(u) => u,
        None => return Ok(None),
    };
    let pool = get_control_pool().await?;

    let stats = db::get_user_ai_stats(&pool, user.id)
        .await
        .map_err(sfn_err)?;

    let tool_usage = db::get_user_tool_usage(&pool, user.id)
        .await
        .map_err(sfn_err)?
        .into_iter()
        .map(|t| ToolUsageInfo {
            ai_tool: t.ai_tool,
            commit_count: t.commit_count,
        })
        .collect();

    let recent_sessions = db::get_user_recent_sessions(&pool, user.id, 5)
        .await
        .map_err(sfn_err)?
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
        .map_err(sfn_err)?;

    let mut risk_map: HashMap<String, i64> = HashMap::new();
    for row in &risk_rows {
        if let Some(ref flags_json) = row.risk_flags
            && let Ok(flags) = serde_json::from_str::<Vec<super::RiskFlagInfo>>(flags_json)
        {
            for flag in flags {
                *risk_map.entry(flag.category).or_insert(0) += 1;
            }
        }
    }
    let risk_counts: Vec<RiskCountInfo> = risk_map
        .into_iter()
        .map(|(category, count)| RiskCountInfo { category, count })
        .collect();

    // Compute user-level vibe score from recent sessions
    let user_vibe_score = {
        let session_data = db::get_user_session_data(&pool, user.id)
            .await
            .unwrap_or_default();
        if session_data.is_empty() {
            None
        } else {
            let mut total_score = 0u64;
            let mut count = 0u64;
            for sd in &session_data {
                // Lightweight score: skip churn (expensive git call), use commits/prompts + basic metrics
                let metrics = vibe::SessionMetrics {
                    commit_count: sd.commit_count as usize,
                    prompt_count: sd.prompt_count as usize,
                    risk_flag_count: 0,
                    files_touched: 0,
                    lines_added: 1,
                    lines_deleted: 0,
                    was_reverted: false,
                };
                let vs = vibe::compute_vibe_score(&metrics);
                total_score += vs.score as u64;
                count += 1;
            }
            Some((total_score / count.max(1)) as u8)
        }
    };

    Ok(Some(DashboardData {
        total_ai_commits: stats.total_ai_commits,
        total_sessions: stats.total_sessions,
        total_repos_with_ai: stats.total_repos_with_ai,
        tool_usage,
        recent_sessions,
        risk_counts,
        user_vibe_score,
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
                        <div class="landing-page">
                            <div class="hero-section">
                                <div class="hero-orb hero-orb-1"></div>
                                <div class="hero-orb hero-orb-2"></div>
                                <div class="hero-content">
                                    <span class="hero-eyebrow hero-stagger">"BETA · THE AI-NATIVE GIT PLATFORM"</span>
                                    <div class="hero-title hero-stagger">
                                        <svg class="hero-logo" viewBox="0 0 84 32" fill="none" xmlns="http://www.w3.org/2000/svg">
                                            <g transform="scale(1.0667)">
                                                <path d="m10.3 10.3-4.8-4.8" stroke="var(--accent)" stroke-linecap="round" stroke-width="2.5"/>
                                                <path d="m21.7 21.7 4.8 4.8" stroke="var(--accent)" stroke-linecap="round" stroke-width="2.5"/>
                                                <g fill="var(--accent)">
                                                    <circle cx="4.5" cy="4.5" r="2.5"/>
                                                    <circle cx="27.5" cy="27.5" r="2.5"/>
                                                    <circle cx="16" cy="16" r="7.5"/>
                                                </g>
                                                <circle cx="16" cy="16" class="logo-hole" r="4.5"/>
                                            </g>
                                            <g fill="var(--text)">
                                                <path d="M30.87 23L27.45 23L31.32 16.83L27.69 10.99L31.17 10.99L32.25 12.88Q32.77 13.82 33.19 14.72L33.19 14.72Q33.29 14.98 33.41 15.23L33.41 15.23Q33.53 14.99 33.65 14.73L33.65 14.73Q34.06 13.82 34.59 12.88L34.59 12.88L35.72 10.99L39.14 10.99L35.43 16.88L39.31 23L35.85 23L34.52 20.74Q33.99 19.82 33.57 18.91L33.57 18.91Q33.46 18.66 33.34 18.41L33.34 18.41Q33.23 18.66 33.12 18.91L33.12 18.91Q32.71 19.82 32.20 20.74L32.20 20.74L30.87 23Z"/>
                                                <path d="M43.92 23L40.70 23L40.70 10.99L43.92 10.99L43.92 23ZM42.31 9.42L42.31 9.42Q41.58 9.42 41.06 8.94Q40.55 8.46 40.55 7.77L40.55 7.77Q40.55 7.08 41.06 6.60Q41.58 6.11 42.31 6.11L42.31 6.11Q43.04 6.11 43.56 6.59Q44.08 7.07 44.08 7.77L44.08 7.77Q44.08 8.46 43.56 8.94Q43.04 9.42 42.31 9.42Z"/>
                                                <path d="M51.56 27.75L51.56 27.75Q50.05 27.75 48.94 27.37Q47.82 26.99 47.12 26.32Q46.42 25.65 46.15 24.80L46.15 24.80L48.93 24.03Q49.08 24.36 49.39 24.68Q49.71 25.00 50.24 25.20Q50.76 25.41 51.54 25.41L51.54 25.41Q52.77 25.41 53.47 24.84Q54.18 24.28 54.18 23.09L54.18 23.09L54.18 20.87L53.93 20.87Q53.72 21.36 53.31 21.81Q52.90 22.26 52.24 22.54Q51.58 22.83 50.60 22.83L50.60 22.83Q49.25 22.83 48.14 22.19Q47.04 21.56 46.37 20.26Q45.71 18.96 45.71 16.97L45.71 16.97Q45.71 14.94 46.39 13.58Q47.06 12.21 48.17 11.53Q49.28 10.84 50.61 10.84L50.61 10.84Q51.62 10.84 52.31 11.18Q53.00 11.52 53.42 12.03Q53.85 12.54 54.06 13.01L54.06 13.01L54.20 13.01L54.20 10.99L57.37 10.99L57.37 22.90Q57.37 24.53 56.62 25.60Q55.88 26.67 54.57 27.21Q53.25 27.75 51.56 27.75ZM51.61 20.38L51.61 20.38Q52.44 20.38 53.01 19.97Q53.59 19.56 53.89 18.79Q54.19 18.03 54.19 16.95L54.19 16.95Q54.19 15.89 53.89 15.09Q53.59 14.30 53.01 13.86Q52.44 13.42 51.61 13.42L51.61 13.42Q50.76 13.42 50.19 13.87Q49.61 14.33 49.32 15.13Q49.02 15.92 49.02 16.95L49.02 16.95Q49.02 18.00 49.32 18.77Q49.61 19.54 50.19 19.96Q50.77 20.38 51.61 20.38Z"/>
                                                <path d="M62.90 23L59.68 23L59.68 10.99L62.90 10.99L62.90 23ZM61.29 9.42L61.29 9.42Q60.56 9.42 60.04 8.94Q59.53 8.46 59.53 7.77L59.53 7.77Q59.53 7.08 60.04 6.60Q60.56 6.11 61.29 6.11L61.29 6.11Q62.02 6.11 62.54 6.59Q63.06 7.07 63.06 7.77L63.06 7.77Q63.06 8.46 62.54 8.94Q62.02 9.42 61.29 9.42Z"/>
                                                <path d="M68.92 10.99L71.16 10.99L71.16 13.45L68.92 13.45L68.92 19.49Q68.92 20.06 69.17 20.33Q69.42 20.60 70.01 20.60L70.01 20.60Q70.20 20.60 70.53 20.56Q70.86 20.51 71.03 20.46L71.03 20.46L71.50 22.88Q70.96 23.04 70.43 23.11Q69.90 23.17 69.41 23.17L69.41 23.17Q67.61 23.17 66.65 22.29Q65.70 21.41 65.70 19.77L65.70 19.77L65.70 13.45L64.04 13.45L64.04 10.99L65.70 10.99L65.70 8.13L68.92 8.13L68.92 10.99Z"/>
                                            </g>
                                        </svg>
                                    </div>
                                    <p class="hero-subtitle hero-stagger">
                                        "The Git platform for vibecoders. Track what your AI builds, review it, remix it, deploy it."
                                    </p>
                                    <div class="hero-actions hero-stagger">
                                        <a href="/login" class="btn btn-lg btn-outline">"Sign in"</a>
                                        <a href="/pricing" class="btn btn-lg btn-primary btn-primary-glow">"Get started"</a>
                                    </div>
                                    <p class="hero-trust hero-stagger">"No credit card required. Free tier available."</p>
                                </div>
                            </div>
                            <div class="hero-features">
                                <div class="feature-card feature-card-stagger">
                                    <div class="feature-card-icon"><IconBranch /></div>
                                    <div class="feature-card-title">"AI-Aware Commits"</div>
                                    <div class="feature-card-desc">"Every commit tracks which AI tool and prompt generated it. Browse your repo as a conversation timeline."</div>
                                </div>
                                <div class="feature-card feature-card-stagger">
                                    <div class="feature-card-icon"><IconSearch /></div>
                                    <div class="feature-card-title">"Smart Diff Review"</div>
                                    <div class="feature-card-desc">"Auto-generated summaries and risk detection on every diff. Understand what your AI wrote, instantly."</div>
                                </div>
                                <div class="feature-card feature-card-stagger">
                                    <div class="feature-card-icon"><IconRepo /></div>
                                    <div class="feature-card-title">"Session Snapshots"</div>
                                    <div class="feature-card-desc">"Browse, review, and revert entire AI coding sessions. The session is the new unit of work."</div>
                                </div>
                                <div class="feature-card feature-card-stagger">
                                    <div class="feature-card-icon"><IconPlus /></div>
                                    <div class="feature-card-title">"Remix Projects"</div>
                                    <div class="feature-card-desc">"One-click remix with starter prompts. Fork a project and start vibecoding on it immediately."</div>
                                </div>
                                <div class="feature-card feature-card-stagger">
                                    <div class="feature-card-icon"><IconServer /></div>
                                    <div class="feature-card-title">"Deploy Previews"</div>
                                    <div class="feature-card-desc">"Webhook-based live previews for every push. See if it works before reading a single line of code."</div>
                                </div>
                                <div class="feature-card feature-card-stagger">
                                    <div class="feature-card-icon"><IconRust /></div>
                                    <div class="feature-card-title">"Self-Hosted Rust"</div>
                                    <div class="feature-card-desc">"Your code, your server. Built with Rust for blazing speed and memory safety. Git over HTTP and SSH."</div>
                                </div>
                            </div>
                            <div class="how-it-works">
                                <div class="how-it-works-title">"How it works"</div>
                                <div class="how-it-works-grid">
                                    <div class="how-step">
                                        <div class="how-step-number">"1"</div>
                                        <div class="how-step-title">"Push your code"</div>
                                        <div class="how-step-desc">"Use git push over HTTP or SSH. Works with any AI coding tool."</div>
                                    </div>
                                    <div class="how-step">
                                        <div class="how-step-number">"2"</div>
                                        <div class="how-step-title">"Oxigit analyzes"</div>
                                        <div class="how-step-desc">"Commits are tagged, sessions grouped, diffs summarized automatically."</div>
                                    </div>
                                    <div class="how-step">
                                        <div class="how-step-number">"3"</div>
                                        <div class="how-step-title">"Review and ship"</div>
                                        <div class="how-step-desc">"Browse sessions, check risk flags, remix projects, deploy previews."</div>
                                    </div>
                                </div>
                            </div>
                            <div class="final-cta">
                                <div class="final-cta-icon"><IconRust /></div>
                                <div class="final-cta-title">"Built with Rust. Ready for your team."</div>
                                <div class="final-cta-desc">"Start free, upgrade when you need AI features and deploy previews."</div>
                                <div class="final-cta-actions">
                                    <a href="/pricing" class="btn btn-lg btn-primary btn-primary-glow">"See plans"</a>
                                    <a href="https://github.com/b-nova/oxigit" target="_blank" rel="noopener noreferrer" class="btn btn-lg btn-outline">"View on GitHub"</a>
                                </div>
                            </div>
                            <div class="landing-footer"></div>
                        </div>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}
