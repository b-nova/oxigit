mod harness;

use harness::*;

/// Skip this test if the server was not built with SaaS features.
async fn require_saas(server: &TestServer) -> bool {
    if !server.has_saas().await {
        eprintln!("SKIPPED: server built without saas feature");
        return false;
    }
    true
}

/// Helper: simulate claiming a founding member slot via Stripe webhook.
async fn upgrade_to_founding(client: &TestClient, base_url: &str, user_id: &str, secret: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_found_{}","subscription":"sub_found_{}","metadata":{{"plan":"founding"}}}}}}}}"#,
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
    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 404,
        "Stripe webhook returned unexpected status: {status}"
    );
}

/// Test: badge SVG endpoint returns valid SVG for a founding member.
#[tokio::test]
async fn badge_svg_returns_valid_svg() {
    let secret = "whsec_badge_svg_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !require_saas(&server).await {
        return;
    }
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_founding(&client, &client.base_url.clone(), "1", secret).await;

    let resp = client
        .client
        .get(format!("{}/api/badge/alice.svg", client.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        200,
        "badge endpoint should return 200"
    );
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("image/svg+xml"),
        "badge should have SVG content type, got: {content_type}"
    );

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("<svg"),
        "badge should contain SVG element: {body}"
    );
    assert!(
        body.contains("FOUNDING"),
        "badge should contain FOUNDING text: {body}"
    );
    assert!(
        body.contains("MEMBER"),
        "badge should contain MEMBER text: {body}"
    );
    assert!(
        body.contains("#001"),
        "first founding member should have slot #001: {body}"
    );
}

/// Test: badge SVG endpoint returns 404 for a non-founding user.
#[tokio::test]
async fn badge_svg_404_for_non_founding_user() {
    let server = TestServer::start().await;
    if !require_saas(&server).await {
        return;
    }
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;

    let resp = client
        .client
        .get(format!("{}/api/badge/bob.svg", client.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        404,
        "badge should return 404 for non-founding user"
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Not a founding member"),
        "404 body should explain reason: {body}"
    );
}

/// Test: badge SVG endpoint returns 404 for a nonexistent user.
#[tokio::test]
async fn badge_svg_404_for_nonexistent_user() {
    let server = TestServer::start().await;
    if !require_saas(&server).await {
        return;
    }
    let client = server.client();

    let resp = client
        .client
        .get(format!("{}/api/badge/nobody.svg", client.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        404,
        "badge should return 404 for nonexistent user"
    );
}

/// Test: founding member profile shows slot number and embed code.
#[tokio::test]
async fn founding_profile_shows_slot_and_embed_code() {
    let secret = "whsec_profile_slot_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !require_saas(&server).await {
        return;
    }
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_founding(&client, &client.base_url.clone(), "1", secret).await;

    let resp = client
        .client
        .get(format!("{}/alice", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("#001"),
        "profile should show slot number #001: {body}"
    );
    assert!(
        body.contains("Download Badge"),
        "profile should have download badge button: {body}"
    );
    assert!(
        body.contains("Embed on your site"),
        "profile should show embed instructions: {body}"
    );
    assert!(
        body.contains("oxigit.com/api/badge/alice.svg"),
        "profile should show embed URL: {body}"
    );
}

/// Test: two founding members get sequential slot numbers.
#[tokio::test]
async fn founding_members_get_sequential_slots() {
    let secret = "whsec_sequential_slots_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !require_saas(&server).await {
        return;
    }

    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_founding(&alice, &alice.base_url.clone(), "1", secret).await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    upgrade_to_founding(&bob, &bob.base_url.clone(), "2", secret).await;

    // Check Alice's badge has #001
    let resp = alice
        .client
        .get(format!("{}/api/badge/alice.svg", alice.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(body.contains("#001"), "alice should have slot #001: {body}");

    // Check Bob's badge has #002
    let resp = bob
        .client
        .get(format!("{}/api/badge/bob.svg", bob.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(body.contains("#002"), "bob should have slot #002: {body}");
}

/// Test: founding member can access AI Hub with full access (not preview mode).
#[tokio::test]
async fn founding_member_can_access_ai_hub() {
    let secret = "whsec_founding_ai_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !require_saas(&server).await {
        return;
    }
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_founding(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/ai", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("Pro or higher plan"),
        "founding member should have full AI Hub access: {body}"
    );
}

/// Test: founding member can use deploy previews (has deploy_previews entitlement).
#[tokio::test]
async fn founding_member_can_use_deploy_previews() {
    let secret = "whsec_founding_deploy_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !require_saas(&server).await {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_founding(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("webapp", "test", false).await;

    // Add webhook so deploy preview record gets created on push
    client
        .add_webhook("alice", "webapp", "https://example.com/deploy", "")
        .await;

    // Push a commit
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "webapp");
    let repo_dir = tmp.path().join("webapp");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(
        &repo_dir,
        "index.html",
        "<h1>Hello</h1>\n",
        "initial commit",
    );
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    // Wait for async post-receive hook to create deploy preview record
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Send deploy callback — should succeed for founding member
    let resp = client
        .client
        .post(format!("{}/api/deploy-callback/{}", client.base_url, sha))
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "repo_owner": "alice",
            "repo_name": "webapp",
            "status": "ready",
            "preview_url": "https://preview.example.com/founding"
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap();
    assert_eq!(
        status, 200,
        "deploy callback should succeed for founding member, got {status}: {body}"
    );
}

/// Test: pricing page shows founding slots remaining after one is claimed.
#[tokio::test]
async fn pricing_page_shows_founding_slots() {
    let secret = "whsec_pricing_slots_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !require_saas(&server).await {
        return;
    }
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_founding(&client, &client.base_url.clone(), "1", secret).await;

    // Fetch pricing info — should show 99 slots remaining
    let resp = client.fetch_pricing_info().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("founding_slots_remaining") || body.contains("99"),
        "pricing info should show founding slots remaining after 1 claimed: {body}"
    );
    assert!(
        !body.contains("\"founding_slots_remaining\":100"),
        "slots should not be 100 after one was claimed: {body}"
    );
}
