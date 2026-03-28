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
use pages::{home::HomePage, login::LoginPage, register::RegisterPage, repo_list::RepoListPage, repo_new::NewRepoPage};

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet href="/pkg/oxigit.css" />
        <Title text="Oxigit" />
        <Meta charset="utf-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1" />

        <Router>
            <Navbar />
            <main class="container">
                <Routes fallback=|| view! { <h1>"404 — Not Found"</h1> }>
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/login") view=LoginPage />
                    <Route path=path!("/register") view=RegisterPage />
                    <Route path=path!("/repos") view=RepoListPage />
                    <Route path=path!("/repos/new") view=NewRepoPage />
                </Routes>
            </main>
        </Router>
    }
}
