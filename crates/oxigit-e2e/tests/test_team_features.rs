mod harness;

use harness::*;

/// Helper: simulate upgrading a user to Team plan via Stripe webhook.
async fn upgrade_to_team(client: &TestClient, base_url: &str, user_id: &str, secret: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_team_{}","subscription":"sub_team_{}","metadata":{{"plan":"team"}}}}}}}}"#,
        user_id, user_id, user_id
    );
    let timestamp = "1234567890";
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let signature = format!("t={},v1={}", timestamp, sig);

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", base_url))
        .header("stripe-signature", &signature)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "team webhook should succeed");
}

/// Test: free user is blocked from org settings (team management).
#[tokio::test]
async fn free_user_blocked_from_org_settings() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    // Create an org via the server function
    let resp = client
        .client
        .post(format!("{}/api/create_org7865708971083396342", client.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("slug=myorg&display_name=My+Org")
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "org creation should succeed, got {}",
        resp.status()
    );

    // Try to access org settings — should be blocked on Free plan
    let resp = client
        .client
        .get(format!("{}/orgs/myorg/settings", client.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Team plan") || body.contains("Upgrade at /pricing"),
        "Org settings should show upgrade prompt for free users: {body}"
    );
}

/// Test: Team user can access org settings.
#[tokio::test]
async fn team_user_can_access_org_settings() {
    let secret = "whsec_team_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_team(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;

    // Create an org
    let resp = client
        .client
        .post(format!("{}/api/create_org7865708971083396342", client.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("slug=myorg&display_name=My+Org")
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "org creation should succeed, got {}",
        resp.status()
    );

    // Access org settings — should work on Team plan
    let resp = client
        .client
        .get(format!("{}/orgs/myorg/settings", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("Team plan") && !body.contains("Upgrade at /pricing"),
        "Team user should access org settings without upgrade prompt: {body}"
    );
    assert!(
        body.contains("Members") || body.contains("Settings"),
        "Org settings page should show member management: {body}"
    );
}
