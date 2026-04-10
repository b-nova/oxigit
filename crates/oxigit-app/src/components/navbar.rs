use leptos::prelude::*;
use leptos_router::hooks::use_location;

use super::icons::{IconGear, IconLogout, IconMenu, IconUser, IconX};
#[cfg(feature = "saas")]
use super::icons::{IconCheck, IconCreditCard, IconReceipt, IconTeam};
use super::theme_toggle::ThemeToggle;
use crate::pages::{get_current_user, Logout};
#[cfg(feature = "saas")]
use crate::pages::{list_my_orgs, SwitchOrg};

#[component]
pub fn Navbar() -> impl IntoView {
    let location = use_location();
    // Refetch user on every route change so navbar updates after login/logout
    let user = Resource::new(
        move || location.pathname.get(),
        |_| get_current_user(),
    );
    let logout_action = ServerAction::<Logout>::new();

    let (menu_open, set_menu_open) = signal(false);

    // Close mobile menu on route change
    Effect::new(move |_| {
        let _ = location.pathname.get();
        set_menu_open.set(false);
    });

    let pathname = move || location.pathname.get();

    let nav_class = move |href: &str| {
        let p = pathname();
        if p == href || (href != "/" && p.starts_with(href)) {
            "navbar-link navbar-link-active"
        } else {
            "navbar-link"
        }
    };

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
                    <a href="/explore" class=move || nav_class("/explore")>"Explore"</a>
                    <a href="/recipes" class=move || nav_class("/recipes")>"Recipes"</a>
                    {
                        #[cfg(feature = "saas")]
                        view! { <a href="/pricing" class=move || nav_class("/pricing")>"Pricing"</a> }
                    }
                    <Suspense fallback=|| ()>
                        {move || Suspend::new(async move {
                            match user.await {
                                Ok(Some(_)) => view! {
                                    <a href="/repos" class=move || nav_class("/repos")>"Repositories"</a>
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
                                    let active_org = u.active_org_slug.clone().unwrap_or_default();
                                    view! {
                                        <UserDropdown
                                            username=u.username.clone()
                                            active_org=active_org
                                            logout_action=logout_action
                                        />
                                    }.into_any()
                                },
                                _ => view! {
                                    <a href="/login" class="navbar-link">"Sign in"</a>
                                    {
                                        #[cfg(feature = "saas")]
                                        view! { <a href="/pricing" class="btn btn-primary btn-sm">"Sign up"</a> }
                                        #[cfg(not(feature = "saas"))]
                                        view! { <a href="/register" class="btn btn-primary btn-sm">"Sign up"</a> }
                                    }
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                    <button
                        class="mobile-nav-toggle"
                        on:click=move |_| set_menu_open.update(|v| *v = !*v)
                        title="Menu"
                    >
                        <IconMenu />
                    </button>
                </div>
            </div>
        </nav>

        // Mobile navigation drawer
        <div class=move || if menu_open.get() { "mobile-nav mobile-nav-open" } else { "mobile-nav" }>
            <div class="mobile-nav-backdrop" on:click=move |_| set_menu_open.set(false)></div>
            <div class="mobile-nav-drawer">
                <div class="mobile-nav-header">
                    <span class="text-secondary" style="font-size: 0.875rem; font-weight: 500;">"Menu"</span>
                    <button class="mobile-nav-close" on:click=move |_| set_menu_open.set(false)>
                        <IconX />
                    </button>
                </div>
                <a href="/explore" class="mobile-nav-link">"Explore"</a>
                <a href="/recipes" class="mobile-nav-link">"Recipes"</a>
                {
                    #[cfg(feature = "saas")]
                    view! { <a href="/pricing" class="mobile-nav-link">"Pricing"</a> }
                }
                <Suspense fallback=|| ()>
                    {move || Suspend::new(async move {
                        match user.await {
                            Ok(Some(_u)) => {
                                view! {
                                    <a href="/repos" class="mobile-nav-link">"Repositories"</a>
                                    <div class="mobile-nav-divider"></div>
                                    <a href="/profile" class="mobile-nav-link">"Profile"</a>
                                    <a href="/settings" class="mobile-nav-link">"Settings"</a>
                                    {
                                        #[cfg(feature = "saas")]
                                        view! {
                                            <a href="/subscription" class="mobile-nav-link">"Subscription"</a>
                                            <a href="/billing" class="mobile-nav-link">"Billing"</a>
                                        }
                                    }
                                    <div class="mobile-nav-divider"></div>
                                    <ActionForm action=logout_action>
                                        <button type="submit" class="mobile-nav-link w-full text-left" style="background: none; border: none; cursor: pointer; font: inherit;">
                                            "Sign out"
                                        </button>
                                    </ActionForm>
                                }.into_any()
                            },
                            _ => view! {
                                <div class="mobile-nav-divider"></div>
                                <a href="/login" class="mobile-nav-link">"Sign in"</a>
                                {
                                    #[cfg(feature = "saas")]
                                    view! { <a href="/pricing" class="mobile-nav-link">"Sign up"</a> }
                                    #[cfg(not(feature = "saas"))]
                                    view! { <a href="/register" class="mobile-nav-link">"Sign up"</a> }
                                }
                            }.into_any(),
                        }
                    })}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn UserDropdown(
    username: String,
    active_org: String,
    logout_action: ServerAction<Logout>,
) -> impl IntoView {
    let (dropdown_open, set_dropdown_open) = signal(false);
    #[cfg(feature = "saas")]
    let orgs = Resource::new(|| (), |_| list_my_orgs());
    #[cfg(feature = "saas")]
    let switch_action = ServerAction::<SwitchOrg>::new();
    let location = use_location();

    // Close dropdown on route change
    Effect::new(move |_| {
        let _ = location.pathname.get();
        set_dropdown_open.set(false);
    });

    #[cfg(feature = "saas")]
    let active_org = StoredValue::new(active_org);
    #[cfg(not(feature = "saas"))]
    let _ = active_org;

    view! {
        <div class="user-dropdown">
            <button
                class="navbar-icon"
                on:click=move |_| set_dropdown_open.update(|v| *v = !*v)
                title=username.clone()
            >
                <IconUser />
            </button>
            <Show when=move || dropdown_open.get()>
                <div
                    class="user-dropdown-backdrop"
                    on:click=move |_| set_dropdown_open.set(false)
                ></div>
                <div class="user-dropdown-menu">
                    {
                        #[cfg(feature = "saas")]
                        view! {
                            <Suspense fallback=|| ()>
                                {move || {
                                    let active = active_org.get_value();
                                    Suspend::new(async move {
                                        match orgs.await {
                                            Ok(orgs) if orgs.len() > 1 => {
                                                let active_slug = active.clone();
                                                let team_href = orgs.iter()
                                                    .find(|o| o.slug == active_slug)
                                                    .or_else(|| orgs.first())
                                                    .map(|o| format!("/orgs/{}/settings", o.slug))
                                                    .unwrap_or_default();
                                                view! {
                                                    <div class="user-dropdown-label">"Organization"</div>
                                                    {orgs.iter().map(|o| {
                                                        let is_active = o.slug == active;
                                                        let slug = o.slug.clone();
                                                        let name = o.display_name.clone();
                                                        view! {
                                                            <ActionForm action=switch_action>
                                                                <input type="hidden" name="slug" value=slug />
                                                                <button
                                                                    type="submit"
                                                                    class="user-dropdown-item"
                                                                    class:user-dropdown-item-active=is_active
                                                                >
                                                                    {name}
                                                                    <Show when=move || is_active>
                                                                        <IconCheck />
                                                                    </Show>
                                                                </button>
                                                            </ActionForm>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                    <a
                                                        href=team_href
                                                        class="user-dropdown-item"
                                                        on:click=move |_| set_dropdown_open.set(false)
                                                    >
                                                        <IconTeam />
                                                        " Team"
                                                    </a>
                                                    <div class="user-dropdown-divider"></div>
                                                }.into_any()
                                            }
                                            Ok(orgs) if orgs.len() == 1 => {
                                                let org = &orgs[0];
                                                let settings_href = format!("/orgs/{}/settings", org.slug);
                                                view! {
                                                    <a
                                                        href=settings_href
                                                        class="user-dropdown-item"
                                                        on:click=move |_| set_dropdown_open.set(false)
                                                    >
                                                        <IconTeam />
                                                        " Team"
                                                    </a>
                                                    <div class="user-dropdown-divider"></div>
                                                }.into_any()
                                            }
                                            _ => view! { <span></span> }.into_any(),
                                        }
                                    })
                                }}
                            </Suspense>
                        }
                    }
                    <a
                        href="/profile"
                        class="user-dropdown-item"
                        on:click=move |_| set_dropdown_open.set(false)
                    >
                        <IconUser />
                        " Profile"
                    </a>
                    <a
                        href="/settings"
                        class="user-dropdown-item"
                        on:click=move |_| set_dropdown_open.set(false)
                    >
                        <IconGear />
                        " Settings"
                    </a>
                    {
                        #[cfg(feature = "saas")]
                        view! {
                            <a
                                href="/subscription"
                                class="user-dropdown-item"
                                on:click=move |_| set_dropdown_open.set(false)
                            >
                                <IconCreditCard />
                                " Subscription"
                            </a>
                            <a
                                href="/billing"
                                class="user-dropdown-item"
                                on:click=move |_| set_dropdown_open.set(false)
                            >
                                <IconReceipt />
                                " Billing"
                            </a>
                        }
                    }
                    <div class="user-dropdown-divider"></div>
                    <ActionForm action=logout_action>
                        <button type="submit" class="user-dropdown-item user-dropdown-item-danger">
                            <IconLogout />
                            " Sign out"
                        </button>
                    </ActionForm>
                </div>
            </Show>
        </div>
    }
}
