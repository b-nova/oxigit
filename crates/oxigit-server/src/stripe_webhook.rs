use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use tracing;

use oxigit_core::{billing, db};

use crate::AppState;

/// POST /api/stripe/webhook
/// Handles incoming Stripe webhook events with signature verification.
pub async fn handle_webhook(
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<AppState>,
    body: Bytes,
) -> Response {
    let webhook_secret = match &state.stripe_webhook_secret {
        Some(s) => s.clone(),
        None => {
            tracing::warn!("Stripe webhook received but STRIPE_WEBHOOK_SECRET not configured");
            return (StatusCode::SERVICE_UNAVAILABLE, "Billing not configured").into_response();
        }
    };

    // Extract and verify signature
    let signature = match headers.get("stripe-signature").and_then(|h| h.to_str().ok()) {
        Some(s) => s.to_string(),
        None => {
            return (StatusCode::BAD_REQUEST, "Missing Stripe-Signature header").into_response();
        }
    };

    if let Err(e) = billing::verify_webhook_signature(&body, &signature, &webhook_secret) {
        tracing::warn!("Stripe webhook signature verification failed: {e}");
        return (StatusCode::BAD_REQUEST, "Invalid signature").into_response();
    }

    // Parse event
    let event = match billing::parse_webhook_event(&body) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to parse Stripe webhook event: {e}");
            return (StatusCode::BAD_REQUEST, "Invalid event payload").into_response();
        }
    };

    tracing::info!("Stripe webhook: {}", event.event_type);

    let pool = &state.pool;

    match event.event_type.as_str() {
        "checkout.session.completed" => {
            let obj = &event.data.object;
            let user_id = obj["client_reference_id"]
                .as_str()
                .and_then(|s| s.parse::<i64>().ok());
            let customer_id = obj["customer"].as_str();
            let subscription_id = obj["subscription"].as_str();

            let (Some(user_id), Some(customer_id)) = (user_id, customer_id) else {
                tracing::error!("checkout.session.completed missing client_reference_id or customer");
                return (StatusCode::OK, "OK").into_response();
            };

            // Determine plan from metadata
            let plan = obj["metadata"]["plan"].as_str().unwrap_or("pro");

            // For founding members, try to claim a slot
            if plan == "founding" {
                match db::claim_founding_slot(pool, user_id).await {
                    Ok(Some(slot)) => {
                        tracing::info!("Founding member slot {slot} claimed by user {user_id}");
                    }
                    Ok(None) => {
                        tracing::warn!("Founding member slots full, user {user_id} gets pro instead");
                        // Fall through — will create subscription as pro instead
                        if let Err(e) = db::upsert_subscription(
                            pool, user_id, customer_id, subscription_id,
                            "pro", "active", None, 1,
                        ).await {
                            tracing::error!("Failed to upsert subscription: {e}");
                        }
                        return (StatusCode::OK, "OK").into_response();
                    }
                    Err(e) => {
                        tracing::error!("Failed to claim founding slot: {e}");
                    }
                }
            }

            if let Err(e) = db::upsert_subscription(
                pool, user_id, customer_id, subscription_id,
                plan, "active", None, 1,
            ).await {
                tracing::error!("Failed to upsert subscription: {e}");
            }
        }

        "customer.subscription.updated" => {
            let obj = &event.data.object;
            let subscription_id = obj["id"].as_str();
            let status = obj["status"].as_str();
            let period_end = obj["current_period_end"].as_i64()
                .map(|ts| chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()))
                .flatten();

            if let (Some(sub_id), Some(status)) = (subscription_id, status) {
                if let Err(e) = db::update_subscription_status(
                    pool, sub_id, status, period_end.as_deref(),
                ).await {
                    tracing::error!("Failed to update subscription status: {e}");
                }
            }
        }

        "customer.subscription.deleted" => {
            let obj = &event.data.object;
            if let Some(sub_id) = obj["id"].as_str() {
                if let Err(e) = db::cancel_subscription(pool, sub_id).await {
                    tracing::error!("Failed to cancel subscription: {e}");
                }
            }
        }

        "invoice.payment_failed" => {
            let obj = &event.data.object;
            if let Some(sub_id) = obj["subscription"].as_str() {
                if let Err(e) = db::update_subscription_status(pool, sub_id, "past_due", None).await {
                    tracing::error!("Failed to mark subscription past_due: {e}");
                }
            }
        }

        _ => {
            tracing::debug!("Unhandled Stripe event type: {}", event.event_type);
        }
    }

    (StatusCode::OK, "OK").into_response()
}
