use leptos::prelude::*;

use crate::components::icons::{IconBranch, IconPlus, IconRepo, IconRust, IconSearch, IconServer};

use super::get_current_user;

#[component]
pub fn HomePage() -> impl IntoView {
    let user = Resource::new(|| (), |_| get_current_user());

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
                        view! {
                            <div class="animate-in">
                                <h1 class="dashboard-greeting">
                                    "Welcome back, " {username}
                                </h1>
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
                            </div>
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
