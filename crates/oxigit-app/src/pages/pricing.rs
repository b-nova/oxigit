use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingCard;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PricingInfo {
    pub is_authenticated: bool,
    pub current_plan: String,
    pub founding_slots_remaining: i64,
    pub stripe_configured: bool,
}

#[server]
async fn fetch_pricing_info() -> Result<PricingInfo, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{
            extract_session_user, get_control_pool, get_stripe_config, sfn_err,
        };
        use oxigit_core::db;

        let user = extract_session_user().await;
        let pool = get_control_pool().await?;
        let stripe = get_stripe_config().await?;

        let current_plan = if let Some(ref u) = user {
            db::get_user_plan(&pool, u.id).await.map_err(sfn_err)?
        } else {
            "free".to_string()
        };

        let founding_count = db::count_founding_members(&pool).await.map_err(sfn_err)?;

        Ok(PricingInfo {
            is_authenticated: user.is_some(),
            current_plan,
            founding_slots_remaining: (100 - founding_count).max(0),
            stripe_configured: stripe.is_some(),
        })
    }
    #[cfg(not(feature = "saas"))]
    {
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[server]
async fn create_checkout(plan: String) -> Result<String, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{
            get_base_url, get_control_pool, get_stripe_config, require_auth, sfn_err,
        };
        use oxigit_core::{billing, db};

        let user = require_auth().await?;
        let pool = get_control_pool().await?;
        let stripe = get_stripe_config()
            .await?
            .ok_or_else(|| ServerFnError::new("Billing not configured"))?;

        let base_url = get_base_url().await;

        // Determine price ID
        let price_id = match plan.as_str() {
            "flat" => stripe
                .price_flat
                .ok_or_else(|| ServerFnError::new("Flat plan price not configured"))?,
            "team" => stripe
                .price_team
                .ok_or_else(|| ServerFnError::new("Team plan price not configured"))?,
            "founding" => {
                // Check founding slots
                let count = db::count_founding_members(&pool).await.map_err(sfn_err)?;
                if count >= 100 {
                    return Err(ServerFnError::new("All founding member slots are taken"));
                }
                stripe
                    .price_founding
                    .ok_or_else(|| ServerFnError::new("Founding plan price not configured"))?
            }
            _ => return Err(ServerFnError::new("Invalid plan")),
        };

        // Get or create Stripe customer
        let sub = db::get_subscription(&pool, user.id)
            .await
            .map_err(sfn_err)?;

        let customer_id = if let Some(ref sub) = sub {
            sub.stripe_customer_id.clone()
        } else {
            // Look up user email
            let db_user = db::get_user_by_id(&pool, user.id).await.map_err(sfn_err)?;
            let cid = billing::create_customer(&stripe.secret_key, &db_user.email, user.id)
                .await
                .map_err(sfn_err)?;
            // Store customer ID with free subscription
            db::upsert_subscription(&pool, user.id, &cid, None, "free", "active", None, 1)
                .await
                .map_err(sfn_err)?;
            cid
        };

        let quantity = if plan == "team" { 3 } else { 1 };

        let checkout_url = billing::create_checkout_session(
            &stripe.secret_key,
            &customer_id,
            &price_id,
            quantity,
            &format!("{}/billing?success=true", base_url),
            &format!("{}/pricing", base_url),
            &user.id.to_string(),
            &plan,
        )
        .await
        .map_err(sfn_err)?;

        Ok(checkout_url)
    }
    #[cfg(not(feature = "saas"))]
    {
        let _ = plan;
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[component]
pub fn PricingPage() -> impl IntoView {
    let pricing = Resource::new(|| (), |_| fetch_pricing_info());

    view! {
        <div class="page-header">
            <h1 class="page-title">"Choose your plan"</h1>
            <p class="page-subtitle">"Start free with AI insights, upgrade to Flat for the full experience."</p>
        </div>

        <Suspense fallback=|| view! { <LoadingCard /> }>
            {move || Suspend::new(async move {
                match pricing.await {
                    Ok(info) => view! {
                        <PricingCards info />
                    }.into_any(),
                    Err(e) => view! {
                        <ErrorDisplay error=e.to_string() />
                    }.into_any(),
                }
            })}
        </Suspense>

        <p class="pricing-selfhost-note">
            "Prefer to self-host? Oxigit is "
            <a href="https://github.com/b-nova/oxigit" target="_blank" rel="noopener">"open source"</a>
            " and free to run on your own infrastructure."
        </p>
    }
}

#[component]
fn PricingCards(info: PricingInfo) -> impl IntoView {
    let info = StoredValue::new(info);
    let query = use_query_map();

    let checkout_action = Action::new(move |plan: &String| {
        let plan = plan.clone();
        async move { create_checkout(plan).await }
    });

    // Redirect to checkout URL when action succeeds
    Effect::new(move || {
        if let Some(Ok(url)) = checkout_action.value().get() {
            let _ = window().location().set_href(&url);
        }
    });

    // Auto-trigger checkout when redirected from register with ?checkout=plan
    Effect::new(move || {
        let checkout_plan = query.read().get("checkout").unwrap_or_default();
        let i = info.get_value();
        if !checkout_plan.is_empty()
            && i.is_authenticated
            && i.stripe_configured
            && matches!(checkout_plan.as_str(), "flat" | "team" | "founding")
        {
            checkout_action.dispatch(checkout_plan);
        }
    });

    let is_loading = Signal::derive(move || checkout_action.pending().get());

    view! {
        <div class="pricing-grid">
            // Free tier
            <div class="card pricing-card">
                <div class="pricing-card-header">
                    <h3>"Free"</h3>
                    <div class="pricing-amount">"$0"</div>
                    <p class="pricing-period">"forever"</p>
                </div>
                <ul class="pricing-features">
                    <li>"Unlimited public repos"</li>
                    <li>"5 private repos"</li>
                    <li>"AI commit badges"</li>
                    <li>"AI Hub preview"</li>
                    <li>"30-day metrics"</li>
                    <li>"Recent prompts (10)"</li>
                    <li>"Community support"</li>
                </ul>
                <div class="pricing-cta">
                    {move || {
                        let i = info.get_value();
                        if i.current_plan == "free" && i.is_authenticated {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                        } else if !i.is_authenticated {
                            view! { <a href="/register" class="btn btn-primary">"Get started"</a> }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </div>
            </div>

            // Flat tier (with Founding toggle)
            {move || {
                let i = info.get_value();
                let slots = i.founding_slots_remaining;
                let show_founding = slots > 0 && i.current_plan != "flat";
                view! {
                    <FlatCard
                        info=i
                        slots_remaining=slots
                        show_founding=show_founding
                        checkout_action=checkout_action
                        is_loading=is_loading
                    />
                }
            }}

            // Team tier
            <div class="card pricing-card">
                <div class="pricing-card-header">
                    <h3>"Team"</h3>
                    <div class="pricing-amount">"$19"<span class="pricing-per">" / user / mo"</span></div>
                    <p class="pricing-period">"min 3 seats, billed monthly"</p>
                </div>
                <ul class="pricing-features">
                    <li>"Everything in Flat"</li>
                    <li>"Team management"</li>
                    <li>"Guardrails & policies"</li>
                    <li>"Recipe marketplace"</li>
                    <li>"Dedicated support"</li>
                </ul>
                <div class="pricing-cta">
                    {move || {
                        let i = info.get_value();
                        if i.current_plan == "team" {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                        } else if !i.stripe_configured {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Coming soon"</span> }.into_any()
                        } else if !i.is_authenticated {
                            view! { <a href="/register?plan=team" class="btn btn-primary">"Start free trial"</a> }.into_any()
                        } else {
                            view! {
                                <button
                                    class="btn btn-primary"
                                    disabled=is_loading
                                    on:click=move |_| { checkout_action.dispatch("team".to_string()); }
                                >{move || if is_loading.get() { "Redirecting..." } else { "Upgrade to Team" }}</button>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            // Enterprise tier
            <div class="card pricing-card">
                <div class="pricing-card-header">
                    <h3>"Enterprise"</h3>
                    <div class="pricing-amount">"Custom"</div>
                    <p class="pricing-period">"tailored to your org"</p>
                </div>
                <ul class="pricing-features">
                    <li>"Everything in Team"</li>
                    <li>"SSO / SAML"</li>
                    <li>"Audit logging"</li>
                    <li>"Custom integrations"</li>
                    <li>"Dedicated account manager"</li>
                    <li>"SLA guarantee"</li>
                </ul>
                <div class="pricing-cta">
                    {move || {
                        let i = info.get_value();
                        if i.current_plan == "enterprise" {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                        } else {
                            view! { <a href="/contact" class="btn btn-primary">"Contact us"</a> }.into_any()
                        }
                    }}
                </div>
            </div>
        </div>

        // Error display
        {move || checkout_action.value().get().and_then(|r| r.err()).map(|e| view! {
            <ErrorDisplay error=e.to_string() />
        })}
    }
}

#[component]
fn FlatCard(
    info: PricingInfo,
    slots_remaining: i64,
    show_founding: bool,
    checkout_action: Action<String, Result<String, ServerFnError>>,
    is_loading: Signal<bool>,
) -> impl IntoView {
    let (is_founding_mode, set_founding_mode) = signal(false);

    view! {
        <div class=move || if is_founding_mode.get() && show_founding {
            "card pricing-card pricing-card-highlight pricing-card-founding-active"
        } else {
            "card pricing-card pricing-card-highlight"
        }>
            <div class="pricing-card-header">
                <h3>{move || if is_founding_mode.get() && show_founding {
                    "Flat — Founding"
                } else {
                    "Flat"
                }}</h3>
                <div class="pricing-amount">
                    {move || if is_founding_mode.get() && show_founding {
                        view! { <>
                            <span class="pricing-amount-old">"$9"</span>
                            " $5"<span class="pricing-per">" / mo"</span>
                        </> }.into_any()
                    } else {
                        view! { <>"$9"<span class="pricing-per">" / mo"</span></> }.into_any()
                    }}
                </div>
                <p class="pricing-period">
                    {move || if is_founding_mode.get() && show_founding {
                        "locked forever — first 100 only"
                    } else {
                        "billed monthly"
                    }}
                </p>
            </div>
            <ul class="pricing-features">
                <li>"Unlimited private repos"</li>
                <li>"AI diff summaries"</li>
                <li>"AI-aware commits"</li>
                <li>"Deploy previews"</li>
                <li>"Priority support"</li>
                {move || (is_founding_mode.get() && show_founding).then(|| view! {
                    <li class="pricing-feature-founding">"Founding member badge"</li>
                    <li class="pricing-feature-founding">"Early access to features"</li>
                    <li class="pricing-feature-founding">"Direct founder support"</li>
                })}
            </ul>
            // Founding Member banner
            {show_founding.then(|| view! {
                <div
                    class=move || if is_founding_mode.get() {
                        "pricing-founding-banner pricing-founding-banner-active"
                    } else {
                        "pricing-founding-banner"
                    }
                    on:click=move |_| set_founding_mode.update(|v| *v = !*v)
                >
                    <div class="pricing-founding-banner-content">
                        <div class="pricing-founding-banner-title">
                            {move || if is_founding_mode.get() {
                                "Founding Member selected"
                            } else {
                                "Become a Founding Member"
                            }}
                        </div>
                        <div class="pricing-founding-banner-desc">
                            {move || if is_founding_mode.get() {
                                "Click to switch back to Standard".to_string()
                            } else {
                                format!("$5/mo locked forever — {} of 100 slots left", slots_remaining)
                            }}
                        </div>
                    </div>
                    <span class="pricing-founding-banner-arrow">
                        {move || if is_founding_mode.get() { "\u{2191}" } else { "\u{2192}" }}
                    </span>
                </div>
            })}
            <div class="pricing-cta">
                {move || {
                    let founding = is_founding_mode.get() && show_founding;
                    let plan_id = if founding { "founding" } else { "flat" };
                    if founding && info.current_plan == "founding" {
                        view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                    } else if !founding && (info.current_plan == "flat" || info.current_plan == "pro") {
                        view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                    } else if !info.stripe_configured {
                        view! { <span class="btn btn-sm" style="opacity:0.6;">"Coming soon"</span> }.into_any()
                    } else if !info.is_authenticated {
                        let href = format!("/register?plan={}", plan_id);
                        if founding {
                            view! {
                                <a href={href} class="btn btn-primary btn-founding">
                                    {format!("Claim your spot ({} left)", slots_remaining)}
                                </a>
                            }.into_any()
                        } else {
                            view! { <a href={href} class="btn btn-primary">"Start free trial"</a> }.into_any()
                        }
                    } else {
                        let plan = plan_id.to_string();
                        if founding {
                            view! {
                                <button
                                    class="btn btn-primary btn-founding"
                                    disabled=is_loading
                                    on:click=move |_| { checkout_action.dispatch("founding".to_string()); }
                                >{move || if is_loading.get() { "Redirecting...".to_string() } else { format!("Claim your spot ({} left)", slots_remaining) }}</button>
                            }.into_any()
                        } else {
                            view! {
                                <button
                                    class="btn btn-primary"
                                    disabled=is_loading
                                    on:click={let plan = plan.clone(); move |_| { checkout_action.dispatch(plan.clone()); }}
                                >{move || if is_loading.get() { "Redirecting..." } else { "Upgrade to Flat" }}</button>
                            }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}
