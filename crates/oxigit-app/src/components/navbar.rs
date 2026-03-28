use leptos::prelude::*;

use crate::pages::get_current_user;

#[component]
pub fn Navbar() -> impl IntoView {
    let user = Resource::new(|| (), |_| get_current_user());

    view! {
        <nav class="navbar">
            <a href="/" class="navbar-brand">"Oxigit"</a>
            <div class="navbar-links">
                <Suspense fallback=|| ()>
                    {move || Suspend::new(async move {
                        match user.await {
                            Ok(Some(u)) => view! {
                                <a href="/repos">"Repositories"</a>
                                <a href="/repos/new" class="btn btn-primary">"New"</a>
                                <span>{u.username}</span>
                                <a href="/logout" class="btn">"Logout"</a>
                            }.into_any(),
                            _ => view! {
                                <a href="/login">"Sign in"</a>
                                <a href="/register" class="btn btn-primary">"Sign up"</a>
                            }.into_any(),
                        }
                    })}
                </Suspense>
            </div>
        </nav>
    }
}
