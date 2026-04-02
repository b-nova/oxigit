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
                        <svg class="navbar-brand-logo" height="34" viewBox="0 0 84 32" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <g transform="scale(1.0667)">
                                <path d="m10.3 10.3-4.8-4.8" stroke="#e08a4a" stroke-linecap="round" stroke-width="2.5"/>
                                <path d="m21.7 21.7 4.8 4.8" stroke="#e08a4a" stroke-linecap="round" stroke-width="2.5"/>
                                <g fill="#e08a4a">
                                    <circle cx="4.5" cy="4.5" r="2.5"/>
                                    <circle cx="27.5" cy="27.5" r="2.5"/>
                                    <circle cx="16" cy="16" r="7.5"/>
                                </g>
                                <circle cx="16" cy="16" fill="#0c0f14" r="4.5"/>
                            </g>
                            <text x="27" y="23" font-family="Inter, -apple-system, BlinkMacSystemFont, sans-serif" font-weight="700" font-size="22" letter-spacing="-0.02em" fill="#e6edf3">{"xigit"}</text>
                        </svg>
                    </a>
                    <span class="navbar-divider"></span>
                    <a href="/explore" class="navbar-link">"Explore"</a>
                    <a href="/recipes" class="navbar-link">"Recipes"</a>
                    <a href="/pricing" class="navbar-link">"Pricing"</a>
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
                                        <a href="/billing" class="navbar-link navbar-link-sm">"Billing"</a>
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
