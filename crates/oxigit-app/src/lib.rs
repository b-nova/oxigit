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
use pages::{
    ai_hub::AiHubPage,
    ai_session_detail::AiSessionDetailPage,
    commit_view::CommitViewPage,
    commits::CommitsPage,
    explore::ExplorePage,
    home::HomePage,
    issue_list::IssueListPage,
    issue_new::IssueNewPage,
    issue_view::IssueViewPage,
    login::LoginPage,
    pr_list::PrListPage,
    pr_new::PrNewPage,
    pr_view::PrViewPage,
    register::RegisterPage,
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
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="" />
                <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet" />
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
            <Navbar />
            <main class="container">
                <Routes fallback=|| view! { <h1>"404 — Not Found"</h1> }>
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/login") view=LoginPage />
                    <Route path=path!("/register") view=RegisterPage />
                    <Route path=path!("/explore") view=ExplorePage />
                    <Route path=path!("/repos") view=RepoListPage />
                    <Route path=path!("/repos/new") view=NewRepoPage />
                    <Route path=path!("/settings") view=SettingsPage />
                    <Route path=path!("/:owner/:repo/issues") view=IssueListPage />
                    <Route path=path!("/:owner/:repo/issues/new") view=IssueNewPage />
                    <Route path=path!("/:owner/:repo/issues/:number") view=IssueViewPage />
                    <Route path=path!("/:owner/:repo/ai") view=AiHubPage />
                    <Route path=path!("/:owner/:repo/ai/:session_id") view=AiSessionDetailPage />
                    <Route path=path!("/:owner/:repo/remix-guide") view=RemixGuidePage />
                    <Route path=path!("/:owner/:repo/commits") view=CommitsPage />
                    <Route path=path!("/:owner/:repo/commit/:sha") view=CommitViewPage />
                    <Route path=path!("/:owner/:repo/settings") view=RepoSettingsPage />
                    <Route path=path!("/:owner/:repo/pulls") view=PrListPage />
                    <Route path=path!("/:owner/:repo/pulls/new") view=PrNewPage />
                    <Route path=path!("/:owner/:repo/pulls/:number") view=PrViewPage />
                    <Route path=path!("/:owner/:repo/blob/*path") view=RepoBlobPage />
                    <Route path=path!("/:owner/:repo/tree/*path") view=RepoViewPage />
                    <Route path=path!("/:owner/:repo") view=RepoViewPage />
                    <Route path=path!("/:user") view=UserProfilePage />
                </Routes>
            </main>
        </Router>
    }
}
