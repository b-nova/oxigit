use leptos::prelude::*;
use leptos_router::hooks::use_location;

use super::icons::{IconGear, IconLogout, IconUser};
use super::theme_toggle::ThemeToggle;
use crate::pages::{get_current_user, list_my_orgs, Logout, SwitchOrg};

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
                                <circle cx="16" cy="16" class="logo-hole" r="4.5"/>
                            </g>
                            <text x="27" y="23" font-family="Inter, -apple-system, BlinkMacSystemFont, sans-serif" font-weight="700" font-size="22" letter-spacing="-0.02em" class="logo-text">{"xigit"}</text>
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
                    <ThemeToggle />
                    <Suspense fallback=|| ()>
                        {move || Suspend::new(async move {
                            match user.await {
                                Ok(Some(u)) => {
                                    let profile_href = format!("/{}", &u.username);
                                    let active_org = u.active_org_slug.clone().unwrap_or_default();
                                    view! {
                                        <OrgSwitcher active_org=active_org />
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

#[component]
fn OrgSwitcher(active_org: String) -> impl IntoView {
    let orgs = Resource::new(|| (), |_| list_my_orgs());
    let switch_action = ServerAction::<SwitchOrg>::new();

    let active_org_signal = active_org;

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let active_org = active_org_signal.clone();
                Suspend::new(async move {
                match orgs.await {
                    Ok(orgs) if orgs.len() > 1 => {
                        let active = active_org.clone();
                        view! {
                            <div class="org-switcher">
                                <ActionForm action=switch_action>
                                    <select
                                        name="slug"
                                        class="org-select"
                                        on:change=move |ev| {
                                            // Submit the form when selection changes
                                            use leptos::wasm_bindgen::JsCast;
                                            if let Some(select) = event_target::<leptos::web_sys::HtmlSelectElement>(&ev)
                                                .closest("form")
                                                .ok()
                                                .flatten()
                                                .and_then(|f| f.dyn_into::<leptos::web_sys::HtmlFormElement>().ok())
                                            {
                                                let _ = select.request_submit();
                                            }
                                        }
                                    >
                                        {orgs.iter().map(|o| {
                                            let selected = o.slug == active;
                                            view! {
                                                <option value={o.slug.clone()} selected=selected>
                                                    {o.display_name.clone()}
                                                </option>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </select>
                                </ActionForm>
                            </div>
                        }.into_any()
                    }
                    Ok(orgs) if orgs.len() == 1 => {
                        let org = &orgs[0];
                        let settings_href = format!("/orgs/{}/settings", org.slug);
                        view! {
                            <a href=settings_href class="navbar-link navbar-link-sm">{org.display_name.clone()}</a>
                        }.into_any()
                    }
                    _ => view! { <span></span> }.into_any(),
                }
            })}}
        </Suspense>
    }
}
