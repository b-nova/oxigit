#![recursion_limit = "256"]

pub mod components;
pub mod pages;

#[cfg(feature = "ssr")]
pub mod server_fns;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use components::navbar::Navbar;
use components::toast::ToastProvider;
use pages::{
    admin::AdminPage,
    ai_hub::AiHubPage,
    ai_session_detail::AiSessionDetailPage,
    billing::BillingPage,
    blame::BlamePage,
    commit_view::CommitViewPage,
    contact::ContactPage,
    conflict_resolve::ConflictResolvePage,
    commits::CommitsPage,
    explore::ExplorePage,
    home::HomePage,
    issue_list::IssueListPage,
    issue_new::IssueNewPage,
    issue_view::IssueViewPage,
    login::LoginPage,
    org_new::OrgNewPage,
    org_settings::OrgSettingsPage,
    pricing::PricingPage,
    pr_list::PrListPage,
    pr_new::PrNewPage,
    pr_view::PrViewPage,
    prompt_detail::PromptDetailPage,
    prompt_history::PromptHistoryPage,
    recipe_detail::RecipeDetailPage,
    recipe_marketplace::RecipeMarketplacePage,
    recipe_share::ShareRecipePage,
    register::RegisterPage,
    repo_metrics::RepoMetricsPage,
    remix_guide::RemixGuidePage,
    repo_blob::RepoBlobPage,
    repo_list::RepoListPage,
    repo_settings::RepoSettingsPage,
    repo_new::NewRepoPage,
    repo_view::RepoViewPage,
    settings::SettingsPage,
    user_profile::UserProfilePage,
};

/// HTML shell — rendered server-side only. Wraps the App component.
#[component]
pub fn Shell(options: LeptosOptions) -> impl IntoView {
    provide_meta_context();
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="" />
                <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet" />
                <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
                <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
                <link rel="manifest" href="/site.webmanifest" />
                <script>"(function(){var t=localStorage.getItem('theme');if(t==='light'||(t!=='dark'&&window.matchMedia('(prefers-color-scheme:light)').matches)){document.documentElement.setAttribute('data-theme','light')}})()"</script>
                <meta name="theme-color" content="#0c0f14" />
                <meta property="og:title" content="Oxigit" />
                <meta property="og:description" content="The AI-native Git platform for vibecoders." />
                <meta property="og:image" content="/og-image.png" />
                <meta name="twitter:card" content="summary_large_image" />
                <Stylesheet href="/pkg/oxigit.css" />
                <Title text="Oxigit" />
                <HydrationScripts options />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

/// App component — hydrated on the client. Contains router and all pages.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Router>
            <ToastProvider>
            <Navbar />
            <main class="container">
                <Routes fallback=|| view! {
                    <div class="not-found">
                        <div class="not-found-code">"404"</div>
                        <div class="not-found-title">"Page not found"</div>
                        <a href="/" class="btn btn-primary">"Go Home"</a>
                    </div>
                }>
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/login") view=LoginPage />
                    <Route path=path!("/register") view=RegisterPage />
                    <Route path=path!("/explore") view=ExplorePage />
                    <Route path=path!("/repos") view=RepoListPage />
                    <Route path=path!("/repos/new") view=NewRepoPage />
                    <Route path=path!("/settings") view=SettingsPage />
                    <Route path=path!("/pricing") view=PricingPage />
                    <Route path=path!("/billing") view=BillingPage />
                    <Route path=path!("/contact") view=ContactPage />
                    <Route path=path!("/admin") view=AdminPage />
                    <Route path=path!("/orgs/new") view=OrgNewPage />
                    <Route path=path!("/orgs/:slug/settings") view=OrgSettingsPage />
                    <Route path=path!("/recipes") view=RecipeMarketplacePage />
                    <Route path=path!("/recipes/:recipe_id") view=RecipeDetailPage />
                    <Route path=path!("/:owner/:repo/issues") view=IssueListPage />
                    <Route path=path!("/:owner/:repo/issues/new") view=IssueNewPage />
                    <Route path=path!("/:owner/:repo/issues/:number") view=IssueViewPage />
                    <Route path=path!("/:owner/:repo/conflicts/:conflict_id") view=ConflictResolvePage />
                    <Route path=path!("/:owner/:repo/metrics") view=RepoMetricsPage />
                    <Route path=path!("/:owner/:repo/prompts") view=PromptHistoryPage />
                    <Route path=path!("/:owner/:repo/ai") view=AiHubPage />
                    <Route path=path!("/:owner/:repo/ai/:session_id/share") view=ShareRecipePage />
                    <Route path=path!("/:owner/:repo/ai/:session_id/prompt/:prompt_index") view=PromptDetailPage />
                    <Route path=path!("/:owner/:repo/ai/:session_id") view=AiSessionDetailPage />
                    <Route path=path!("/:owner/:repo/remix-guide") view=RemixGuidePage />
                    <Route path=path!("/:owner/:repo/commits") view=CommitsPage />
                    <Route path=path!("/:owner/:repo/commit/:sha") view=CommitViewPage />
                    <Route path=path!("/:owner/:repo/settings") view=RepoSettingsPage />
                    <Route path=path!("/:owner/:repo/pulls") view=PrListPage />
                    <Route path=path!("/:owner/:repo/pulls/new") view=PrNewPage />
                    <Route path=path!("/:owner/:repo/pulls/:number") view=PrViewPage />
                    <Route path=path!("/:owner/:repo/blame/*path") view=BlamePage />
                    <Route path=path!("/:owner/:repo/blob/*path") view=RepoBlobPage />
                    <Route path=path!("/:owner/:repo/tree/*path") view=RepoViewPage />
                    <Route path=path!("/:owner/:repo") view=RepoViewPage />
                    <Route path=path!("/:user") view=UserProfilePage />
                </Routes>
            </main>
            </ToastProvider>
        </Router>
    }
}
