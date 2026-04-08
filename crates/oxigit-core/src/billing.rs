use serde::Deserialize;

use crate::error::{OxigitError, Result};

const STRIPE_API_BASE: &str = "https://api.stripe.com/v1";

/// Create a Stripe customer for a user.
/// Returns the Stripe customer ID.
pub async fn create_customer(
    stripe_key: &str,
    email: &str,
    user_id: i64,
) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/customers", STRIPE_API_BASE))
        .basic_auth(stripe_key, None::<&str>)
        .form(&[
            ("email", email),
            ("metadata[oxigit_user_id]", &user_id.to_string()),
        ])
        .send()
        .await
        .map_err(|e| OxigitError::Billing(format!("Failed to create Stripe customer: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OxigitError::Billing(format!("Stripe create customer failed: {body}")));
    }

    let body: StripeCustomer = resp.json().await
        .map_err(|e| OxigitError::Billing(format!("Failed to parse Stripe response: {e}")))?;
    Ok(body.id)
}

/// Create a Stripe Checkout session.
/// Returns the checkout session URL for client-side redirect.
pub async fn create_checkout_session(
    stripe_key: &str,
    customer_id: &str,
    price_id: &str,
    quantity: i64,
    success_url: &str,
    cancel_url: &str,
    client_reference_id: &str,
    plan: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/checkout/sessions", STRIPE_API_BASE))
        .basic_auth(stripe_key, None::<&str>)
        .form(&[
            ("customer", customer_id),
            ("mode", "subscription"),
            ("line_items[0][price]", price_id),
            ("line_items[0][quantity]", &quantity.to_string()),
            ("success_url", success_url),
            ("cancel_url", cancel_url),
            ("client_reference_id", client_reference_id),
            ("metadata[plan]", plan),
        ])
        .send()
        .await
        .map_err(|e| OxigitError::Billing(format!("Failed to create checkout session: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OxigitError::Billing(format!("Stripe checkout session failed: {body}")));
    }

    let body: StripeCheckoutSession = resp.json().await
        .map_err(|e| OxigitError::Billing(format!("Failed to parse checkout response: {e}")))?;
    body.url.ok_or_else(|| OxigitError::Billing("Checkout session has no URL".into()))
}

/// Create a Stripe Customer Portal session.
/// Returns the portal URL for client-side redirect.
pub async fn create_portal_session(
    stripe_key: &str,
    customer_id: &str,
    return_url: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/billing_portal/sessions", STRIPE_API_BASE))
        .basic_auth(stripe_key, None::<&str>)
        .form(&[
            ("customer", customer_id),
            ("return_url", return_url),
        ])
        .send()
        .await
        .map_err(|e| OxigitError::Billing(format!("Failed to create portal session: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(OxigitError::Billing(format!("Stripe portal session failed: {body}")));
    }

    let body: StripePortalSession = resp.json().await
        .map_err(|e| OxigitError::Billing(format!("Failed to parse portal response: {e}")))?;
    Ok(body.url)
}

/// Verify a Stripe webhook signature using HMAC-SHA256.
/// Stripe sends `t=<timestamp>,v1=<signature>` in the Stripe-Signature header.
pub fn verify_webhook_signature(
    payload: &[u8],
    signature_header: &str,
    webhook_secret: &str,
) -> Result<()> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut timestamp = None;
    let mut signatures = Vec::new();

    for part in signature_header.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t);
        } else if let Some(v1) = part.strip_prefix("v1=") {
            signatures.push(v1);
        }
    }

    let timestamp = timestamp
        .ok_or_else(|| OxigitError::Billing("Missing timestamp in Stripe signature".into()))?;

    if signatures.is_empty() {
        return Err(OxigitError::Billing("No v1 signature found".into()));
    }

    // Construct the signed payload: "timestamp.payload"
    let signed_payload = format!("{}.{}", timestamp, std::str::from_utf8(payload)
        .map_err(|_| OxigitError::Billing("Invalid UTF-8 in webhook payload".into()))?);

    let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
        .map_err(|_| OxigitError::Billing("Invalid webhook secret".into()))?;
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    // Check if any v1 signature matches
    if signatures.iter().any(|sig| *sig == expected) {
        Ok(())
    } else {
        Err(OxigitError::Billing("Webhook signature verification failed".into()))
    }
}

/// Parse a Stripe webhook event from raw JSON payload.
pub fn parse_webhook_event(payload: &[u8]) -> Result<StripeEvent> {
    serde_json::from_slice(payload)
        .map_err(|e| OxigitError::Billing(format!("Failed to parse webhook event: {e}")))
}

// --- Stripe API response types (minimal) ---

#[derive(Deserialize)]
struct StripeCustomer {
    id: String,
}

#[derive(Deserialize)]
struct StripeCheckoutSession {
    url: Option<String>,
}

#[derive(Deserialize)]
struct StripePortalSession {
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StripeEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: StripeEventData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StripeEventData {
    pub object: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_webhook_signature_valid() {
        let secret = "whsec_test_secret";
        let payload = b"{\"id\":\"evt_test\"}";
        let timestamp = "1234567890";

        // Compute expected signature
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let signed = format!("{}.{}", timestamp, std::str::from_utf8(payload).unwrap());
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());

        let header = format!("t={},v1={}", timestamp, sig);
        assert!(verify_webhook_signature(payload, &header, secret).is_ok());
    }

    #[test]
    fn test_verify_webhook_signature_invalid() {
        let secret = "whsec_test_secret";
        let payload = b"{\"id\":\"evt_test\"}";
        let header = "t=1234567890,v1=invalidsignature";
        assert!(verify_webhook_signature(payload, &header, secret).is_err());
    }

    #[test]
    fn test_verify_webhook_signature_missing_timestamp() {
        let secret = "whsec_test_secret";
        let payload = b"{\"id\":\"evt_test\"}";
        let header = "v1=somesig";
        assert!(verify_webhook_signature(payload, &header, secret).is_err());
    }

    #[test]
    fn test_parse_webhook_event() {
        let payload = br#"{"type":"checkout.session.completed","data":{"object":{"id":"cs_test"}}}"#;
        let event = parse_webhook_event(payload).unwrap();
        assert_eq!(event.event_type, "checkout.session.completed");
    }
}
