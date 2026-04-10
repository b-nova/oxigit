use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::error_display::ErrorDisplay;
use crate::components::loading::LoadingCard;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingInfo {
    pub plan: String,
    pub status: String,
    pub current_period_end: Option<String>,
    pub seats: i64,
    pub has_stripe: bool,
}

#[server]
async fn fetch_billing_info() -> Result<BillingInfo, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{sfn_err, extract_session_user, get_control_pool, get_stripe_config};
        use oxigit_core::db;

        let user = extract_session_user().await
            .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
        let pool = get_control_pool().await?;
        let stripe = get_stripe_config().await?;

        let sub = db::get_subscription(&pool, user.id).await
            .map_err(sfn_err)?;

        match sub {
            Some(s) => Ok(BillingInfo {
                plan: s.plan,
                status: s.status,
                current_period_end: s.current_period_end,
                seats: s.seats,
                has_stripe: stripe.is_some(),
            }),
            None => Ok(BillingInfo {
                plan: "free".to_string(),
                status: "active".to_string(),
                current_period_end: None,
                seats: 1,
                has_stripe: stripe.is_some(),
            }),
        }
    }
    #[cfg(not(feature = "saas"))]
    {
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[server]
async fn create_portal_redirect() -> Result<String, ServerFnError> {
    #[cfg(feature = "saas")]
    {
        use crate::server_fns::{sfn_err, extract_session_user, get_base_url, get_control_pool, get_stripe_config};
        use oxigit_core::{billing, db};

        let user = extract_session_user().await
            .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
        let pool = get_control_pool().await?;
        let stripe = get_stripe_config().await?
            .ok_or_else(|| ServerFnError::new("Billing not configured"))?;

        let sub = db::get_subscription(&pool, user.id).await
            .map_err(sfn_err)?
            .ok_or_else(|| ServerFnError::new("No subscription found"))?;

        let base_url = get_base_url().await;

        let portal_url = billing::create_portal_session(
            &stripe.secret_key,
            &sub.stripe_customer_id,
            &format!("{}/subscription", base_url),
        ).await.map_err(sfn_err)?;

        Ok(portal_url)
    }
    #[cfg(not(feature = "saas"))]
    {
        Err(ServerFnError::new("This feature requires the SaaS edition"))
    }
}

#[component]
pub fn SubscriptionPage() -> impl IntoView {
    let billing = Resource::new(|| (), |_| fetch_billing_info());

    let portal_action = Action::new(move |_: &()| async move {
        create_portal_redirect().await
    });

    // Redirect when portal URL is ready
    Effect::new(move || {
        if let Some(Ok(url)) = portal_action.value().get() {
            let _ = window().location().set_href(&url);
        }
    });

    view! {
        <div class="page-header">
            <h1 class="page-title">"Subscription"</h1>
        </div>

        <Suspense fallback=|| view! { <LoadingCard /> }>
            {move || Suspend::new(async move {
                match billing.await {
                    Ok(info) => {
                        let plan_display = match info.plan.as_str() {
                            "flat" => "Flat",
                            "team" => "Team",
                            "founding" => "Founding Member",
                            "enterprise" => "Enterprise",
                            _ => "Free",
                        };

                        let status_class = match info.status.as_str() {
                            "active" => "badge-success",
                            "past_due" => "badge-warning",
                            "canceled" => "badge-danger",
                            _ => "",
                        };

                        let has_stripe = info.has_stripe;
                        let has_paid_sub = info.plan != "free" && has_stripe;
                        let is_loading = move || portal_action.pending().get();

                        view! {
                            <div class="card" style="max-width: 600px;">
                                <div class="card-body">
                                    <div class="billing-row">
                                        <span class="billing-label">"Plan"</span>
                                        <span class="billing-value">{plan_display}</span>
                                    </div>
                                    <div class="billing-row">
                                        <span class="billing-label">"Status"</span>
                                        <span class={format!("badge {}", status_class)}>{info.status.clone()}</span>
                                    </div>
                                    {(info.plan == "team" || info.plan == "enterprise").then(|| view! {
                                        <div class="billing-row">
                                            <span class="billing-label">"Seats"</span>
                                            <span class="billing-value">{info.seats}</span>
                                        </div>
                                    })}
                                    {info.current_period_end.clone().map(|end| view! {
                                        <div class="billing-row">
                                            <span class="billing-label">"Current period ends"</span>
                                            <span class="billing-value">{end}</span>
                                        </div>
                                    })}

                                    {info.status.clone().eq("past_due").then(|| view! {
                                        <div class="flash flash-warning" style="margin-top: var(--space-3);">
                                            "Your payment is past due. Please update your payment method to avoid service interruption."
                                        </div>
                                    })}
                                </div>

                                <div class="card-footer" style="display: flex; gap: var(--space-3);">
                                    {has_paid_sub.then(|| view! {
                                        <button
                                            class="btn btn-primary"
                                            disabled=is_loading
                                            on:click=move |_| { portal_action.dispatch(()); }
                                        >{move || if is_loading() { "Redirecting..." } else { "Manage billing" }}</button>
                                    })}
                                    <a href="/pricing" class="btn">"View plans"</a>
                                </div>
                            </div>

                            {move || portal_action.value().get().and_then(|r| r.err()).map(|e| view! {
                                <div class="flash flash-error" style="margin-top: var(--space-4);">{e.to_string()}</div>
                            })}
                        }.into_any()
                    },
                    Err(e) => view! {
                        <ErrorDisplay error=e.to_string() />
                    }.into_any(),
                }
            })}
        </Suspense>
    }
}
