use leptos::prelude::*;
use leptos_router::hooks::use_location;

use super::icons::{IconGear, IconLogout, IconUser};
use crate::pages::{get_current_user, Logout};

#[component]
pub fn Navbar() -> impl IntoView {
    let location = use_location();
    // Refetch user on every route change so navbar updates after login/logout
    let user = Resource::new(
        move || location.pathname.get(),
        |_| get_current_user(),
    );
    let logout_action = ServerAction::<Logout>::new();

    view! {
        <nav class="navbar">
            <div class="navbar-inner">
                <div class="navbar-nav">
                    <a href="/" class="navbar-brand">
                        <span class="navbar-brand-accent">"O"</span>"xigit"
                    </a>
                    <span class="navbar-divider"></span>
                    <a href="/explore" class="navbar-link">"Explore"</a>
                    <a href="/recipes" class="navbar-link">"Recipes"</a>
                    <Suspense fallback=|| ()>
                        {move || Suspend::new(async move {
                            match user.await {
                                Ok(Some(_)) => view! {
                                    <a href="/repos" class="navbar-link">"Repositories"</a>
                                }.into_any(),
                                _ => view! { <span></span> }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>
                <div class="navbar-actions">
                    <Suspense fallback=|| ()>
                        {move || Suspend::new(async move {
                            match user.await {
                                Ok(Some(u)) => {
                                    let profile_href = format!("/{}", &u.username);
                                    view! {
                                        <a href="/settings" class="navbar-icon" title="Settings">
                                            <IconGear />
                                        </a>
                                        <a href=profile_href class="navbar-icon" title=u.username>
                                            <IconUser />
                                        </a>
                                        <span class="navbar-divider"></span>
                                        <ActionForm action=logout_action>
                                            <button type="submit" class="navbar-icon" title="Logout">
                                                <IconLogout />
                                            </button>
                                        </ActionForm>
                                    }.into_any()
                                },
                                _ => view! {
                                    <a href="/login" class="navbar-link">"Sign in"</a>
                                    <a href="/register" class="btn btn-primary btn-sm">"Sign up"</a>
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>
            </div>
        </nav>
    }
}
