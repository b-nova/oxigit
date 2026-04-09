mod harness;

use harness::*;

/// Helper: compute Stripe webhook signature for test payloads.
fn sign_webhook(payload: &str, secret: &str, timestamp: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("t={},v1={}", timestamp, sig)
}

/// Test: pricing page returns 200 and contains plan names.
#[tokio::test]
async fn pricing_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client
        .client
        .get(format!("{}/pricing", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("Free"), "pricing page should show Free plan: {body}");
    assert!(body.contains("Flat"), "pricing page should show Flat plan: {body}");
    assert!(body.contains("Team"), "pricing page should show Team plan: {body}");
    assert!(body.contains("Founding Member"), "pricing page should show Founding Member: {body}");
}

/// Test: subscription page redirects unauthenticated users (shows error or login prompt).
#[tokio::test]
async fn subscription_page_requires_auth() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client
        .client
        .get(format!("{}/subscription", client.base_url))
        .send()
        .await
        .unwrap();

    // The page should render but show "Not authenticated" error
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Not authenticated") || body.contains("Sign in"),
        "subscription page should indicate auth required: {body}"
    );
}

/// Test: subscription page returns 200 for authenticated users.
#[tokio::test]
async fn subscription_page_shows_for_authenticated_user() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    let resp = client
        .client
        .get(format!("{}/subscription", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("Free") || body.contains("Subscription"), "subscription page should show plan info: {body}");
}

/// Test: webhook rejects requests without Stripe-Signature header.
#[tokio::test]
async fn webhook_rejects_missing_signature() {
    let server = TestServer::start_with_stripe("whsec_test_secret").await;
    let client = server.client();

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", client.base_url))
        .body("{}")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 400);
}

/// Test: webhook rejects invalid signature.
#[tokio::test]
async fn webhook_rejects_invalid_signature() {
    let server = TestServer::start_with_stripe("whsec_test_secret").await;
    let client = server.client();

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", client.base_url))
        .header("stripe-signature", "t=12345,v1=invalidsignature")
        .body("{\"type\":\"checkout.session.completed\",\"data\":{\"object\":{}}}")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 400);
}

/// Test: webhook accepts valid signature and processes checkout.session.completed.
#[tokio::test]
async fn webhook_processes_checkout_completed() {
    let secret = "whsec_test_secret";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    // Register a user first (user_id will be 1)
    client.register("alice", "alice@test.com", "password123").await;

    let payload = r#"{"type":"checkout.session.completed","data":{"object":{"client_reference_id":"1","customer":"cus_test123","subscription":"sub_test123","metadata":{"plan":"flat"}}}}"#;
    let timestamp = "1234567890";
    let signature = sign_webhook(payload, secret, timestamp);

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", client.base_url))
        .header("stripe-signature", &signature)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);

    // Verify the subscription page now shows Flat plan
    client.login("alice", "password123").await;
    let resp = client
        .client
        .get(format!("{}/subscription", client.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(body.contains("Flat"), "subscription page should show Flat plan after checkout: {body}");
}

/// Test: webhook handles subscription.deleted by canceling.
#[tokio::test]
async fn webhook_handles_subscription_deleted() {
    let secret = "whsec_test_secret";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;

    // First create a subscription via checkout
    let payload = r#"{"type":"checkout.session.completed","data":{"object":{"client_reference_id":"1","customer":"cus_test456","subscription":"sub_test456","metadata":{"plan":"flat"}}}}"#;
    let timestamp = "1234567890";
    let signature = sign_webhook(payload, secret, timestamp);

    client
        .client
        .post(format!("{}/api/stripe/webhook", client.base_url))
        .header("stripe-signature", &signature)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await
        .unwrap();

    // Now delete the subscription
    let payload2 = r#"{"type":"customer.subscription.deleted","data":{"object":{"id":"sub_test456"}}}"#;
    let timestamp2 = "1234567891";
    let signature2 = sign_webhook(payload2, secret, timestamp2);

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", client.base_url))
        .header("stripe-signature", &signature2)
        .header("content-type", "application/json")
        .body(payload2)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);

    // Verify subscription shows canceled/free
    client.login("alice", "password123").await;
    let resp = client
        .client
        .get(format!("{}/subscription", client.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("canceled") || body.contains("Free"),
        "subscription should show canceled status after deletion: {body}"
    );
}

/// Test: webhook returns 503 when Stripe is not configured.
#[tokio::test]
async fn webhook_returns_503_when_not_configured() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", client.base_url))
        .header("stripe-signature", "t=12345,v1=test")
        .body("{}")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 503);
}
