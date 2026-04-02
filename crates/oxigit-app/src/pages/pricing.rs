use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PricingInfo {
    pub is_authenticated: bool,
    pub current_plan: String,
    pub founding_slots_remaining: i64,
    pub stripe_configured: bool,
}

#[server]
async fn fetch_pricing_info() -> Result<PricingInfo, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_pool, get_stripe_config};
    use oxigit_core::db;

    let user = extract_session_user().await;
    let pool = get_pool().await?;
    let stripe = get_stripe_config().await?;

    let current_plan = if let Some(ref u) = user {
        db::get_user_plan(&pool, u.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        "free".to_string()
    };

    let founding_count = db::count_founding_members(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(PricingInfo {
        is_authenticated: user.is_some(),
        current_plan,
        founding_slots_remaining: (100 - founding_count).max(0),
        stripe_configured: stripe.is_some(),
    })
}

#[server]
async fn create_checkout(plan: String) -> Result<String, ServerFnError> {
    use crate::server_fns::{extract_session_user, get_base_url, get_pool, get_stripe_config};
    use oxigit_core::{billing, db};

    let user = extract_session_user().await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = get_pool().await?;
    let stripe = get_stripe_config().await?
        .ok_or_else(|| ServerFnError::new("Billing not configured"))?;

    let base_url = get_base_url().await;

    // Determine price ID
    let price_id = match plan.as_str() {
        "pro" => stripe.price_pro
            .ok_or_else(|| ServerFnError::new("Pro plan price not configured"))?,
        "team" => stripe.price_team
            .ok_or_else(|| ServerFnError::new("Team plan price not configured"))?,
        "founding" => {
            // Check founding slots
            let count = db::count_founding_members(&pool).await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            if count >= 100 {
                return Err(ServerFnError::new("All founding member slots are taken"));
            }
            stripe.price_founding
                .ok_or_else(|| ServerFnError::new("Founding plan price not configured"))?
        }
        _ => return Err(ServerFnError::new("Invalid plan")),
    };

    // Get or create Stripe customer
    let sub = db::get_subscription(&pool, user.id).await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let customer_id = if let Some(ref sub) = sub {
        sub.stripe_customer_id.clone()
    } else {
        // Look up user email
        let db_user = db::get_user_by_id(&pool, user.id).await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let cid = billing::create_customer(&stripe.secret_key, &db_user.email, user.id).await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        // Store customer ID with free subscription
        db::upsert_subscription(&pool, user.id, &cid, None, "free", "active", None, 1).await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
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
    ).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(checkout_url)
}

#[component]
pub fn PricingPage() -> impl IntoView {
    let pricing = Resource::new(|| (), |_| fetch_pricing_info());

    view! {
        <div class="page-header">
            <h1 class="page-title">"Pricing"</h1>
            <p class="page-subtitle">"Simple, transparent pricing for every team."</p>
        </div>

        <Suspense fallback=|| view! { <p class="empty-state">"Loading..."</p> }>
            {move || Suspend::new(async move {
                match pricing.await {
                    Ok(info) => view! {
                        <PricingCards info />
                    }.into_any(),
                    Err(e) => view! {
                        <div class="flash flash-error">{e.to_string()}</div>
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}

#[component]
fn PricingCards(info: PricingInfo) -> impl IntoView {
    let info = StoredValue::new(info);

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

    let is_loading = move || checkout_action.pending().get();

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
                    <li>"Basic git hosting"</li>
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

            // Pro tier
            <div class="card pricing-card pricing-card-highlight">
                <div class="pricing-card-header">
                    <h3>"Pro"</h3>
                    <div class="pricing-amount">"$9"<span class="pricing-per">" / user / mo"</span></div>
                    <p class="pricing-period">"billed monthly"</p>
                </div>
                <ul class="pricing-features">
                    <li>"Unlimited private repos"</li>
                    <li>"AI diff summaries"</li>
                    <li>"AI-aware commits"</li>
                    <li>"Deploy previews"</li>
                    <li>"Priority support"</li>
                </ul>
                <div class="pricing-cta">
                    {move || {
                        let i = info.get_value();
                        if i.current_plan == "pro" {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                        } else if !i.is_authenticated {
                            view! { <a href="/register?plan=pro" class="btn btn-primary">"Start free trial"</a> }.into_any()
                        } else if i.stripe_configured {
                            view! {
                                <button
                                    class="btn btn-primary"
                                    disabled=is_loading
                                    on:click=move |_| { checkout_action.dispatch("pro".to_string()); }
                                >{move || if is_loading() { "Redirecting..." } else { "Upgrade to Pro" }}</button>
                            }.into_any()
                        } else {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Coming soon"</span> }.into_any()
                        }
                    }}
                </div>
            </div>

            // Team tier
            <div class="card pricing-card">
                <div class="pricing-card-header">
                    <h3>"Team"</h3>
                    <div class="pricing-amount">"$19"<span class="pricing-per">" / user / mo"</span></div>
                    <p class="pricing-period">"min 3 seats, billed monthly"</p>
                </div>
                <ul class="pricing-features">
                    <li>"Everything in Pro"</li>
                    <li>"Team management"</li>
                    <li>"Guardrails & policies"</li>
                    <li>"Recipe marketplace"</li>
                    <li>"Audit log"</li>
                    <li>"Dedicated support"</li>
                </ul>
                <div class="pricing-cta">
                    {move || {
                        let i = info.get_value();
                        if i.current_plan == "team" {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                        } else if !i.is_authenticated {
                            view! { <a href="/register?plan=team" class="btn btn-primary">"Start free trial"</a> }.into_any()
                        } else if i.stripe_configured {
                            view! {
                                <button
                                    class="btn btn-primary"
                                    disabled=is_loading
                                    on:click=move |_| { checkout_action.dispatch("team".to_string()); }
                                >{move || if is_loading() { "Redirecting..." } else { "Upgrade to Team" }}</button>
                            }.into_any()
                        } else {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Coming soon"</span> }.into_any()
                        }
                    }}
                </div>
            </div>

            // Founding Member
            <div class="card pricing-card pricing-card-founding">
                <div class="pricing-card-header">
                    <h3>"Founding Member"</h3>
                    <div class="pricing-amount">"$5"<span class="pricing-per">" / user / mo"</span></div>
                    <p class="pricing-period">"locked forever, first 100 only"</p>
                </div>
                <ul class="pricing-features">
                    <li>"Everything in Pro"</li>
                    <li>"Locked-in rate forever"</li>
                    <li>"Founding member badge"</li>
                    <li>"Early access to features"</li>
                    <li>"Direct founder support"</li>
                </ul>
                <div class="pricing-cta">
                    {move || {
                        let i = info.get_value();
                        let slots = i.founding_slots_remaining;
                        if i.current_plan == "founding" {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Current plan"</span> }.into_any()
                        } else if slots == 0 {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Sold out"</span> }.into_any()
                        } else if !i.is_authenticated {
                            view! {
                                <a href="/register?plan=founding" class="btn btn-primary">
                                    {format!("Join ({} slots left)", slots)}
                                </a>
                            }.into_any()
                        } else if i.stripe_configured {
                            view! {
                                <button
                                    class="btn btn-primary"
                                    disabled=is_loading
                                    on:click=move |_| { checkout_action.dispatch("founding".to_string()); }
                                >{move || if is_loading() { "Redirecting...".to_string() } else { format!("Join ({} slots left)", slots) }}</button>
                            }.into_any()
                        } else {
                            view! { <span class="btn btn-sm" style="opacity:0.6;">"Coming soon"</span> }.into_any()
                        }
                    }}
                </div>
            </div>
        </div>

        // Error display
        {move || checkout_action.value().get().and_then(|r| r.err()).map(|e| view! {
            <div class="flash flash-error" style="margin-top: var(--space-4);">{e.to_string()}</div>
        })}
    }
}
