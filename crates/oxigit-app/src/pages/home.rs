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
                                "A self-hosted Git platform, built with Rust. Fast, lightweight, and fully under your control."
                            </p>
                            <div class="hero-actions">
                                <a href="/login" class="btn btn-lg btn-outline">"Sign in"</a>
                                <a href="/register" class="btn btn-lg btn-primary">"Get started"</a>
                            </div>
                            <div class="hero-features">
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconServer /></div>
                                    <div class="feature-card-title">"Self-Hosted"</div>
                                    <div class="feature-card-desc">"Your code, your server. Full control over your data and infrastructure."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconRust /></div>
                                    <div class="feature-card-title">"Built with Rust"</div>
                                    <div class="feature-card-desc">"Blazing fast performance with memory safety. No garbage collector, no compromises."</div>
                                </div>
                                <div class="feature-card">
                                    <div class="feature-card-icon"><IconBranch /></div>
                                    <div class="feature-card-title">"Git Native"</div>
                                    <div class="feature-card-desc">"Full Git protocol support over HTTP and SSH. Works with every Git client."</div>
                                </div>
                            </div>
                        </div>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}
