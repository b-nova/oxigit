mod harness;

use harness::*;

/// Helper: simulate upgrading a user to Pro (flat) plan via Stripe webhook.
async fn upgrade_to_pro(client: &TestClient, base_url: &str, user_id: &str, secret: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_pro_{}","subscription":"sub_pro_{}","metadata":{{"plan":"flat"}}}}}}}}"#,
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
    assert!(status == 200 || status == 404, "Stripe webhook returned unexpected status: {status}");
}

/// Test that the session detail page shows squash and cherry-pick buttons.
#[tokio::test]
async fn test_session_detail_shows_operations() {
    let secret = "whsec_session_ops_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("opsrepo", "Session ops test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "opsrepo");
    let dest = server.data_dir.path().join("clone-ops");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Create a branch so cherry-pick has a target
    create_commit(&dest, "base.txt", "base", "initial");
    create_branch(&dest, "feature");

    // Create AI session commits
    create_commit_with_trailers(
        &dest, "feat.rs", "fn feat() {}", "step 1",
        "claude-code", Some("claude-opus-4-6"), Some("Build the feature"),
        Some("ops-session-1"), Some(1),
    );
    create_commit_with_trailers(
        &dest, "feat_test.rs", "fn test_feat() {}", "step 2",
        "claude-code", Some("claude-opus-4-6"), Some("Add tests"),
        Some("ops-session-1"), Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch session detail page
    let resp = client.get("/alice/opsrepo/ai/ops-session-1").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Session Operations"), "Expected operations panel, got: {}", &body[..500.min(body.len())]);
    assert!(body.contains("Squash Session"), "Expected squash button");
    assert!(body.contains("Cherry-pick to"), "Expected cherry-pick button");
    assert!(body.contains("Revert Session"), "Expected revert button");
}

/// Test that session detail page shows correct commit count and prompts.
#[tokio::test]
async fn test_session_detail_content() {
    let secret = "whsec_session_content_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("bob", "password123").await;
    client.create_repo("detailrepo", "Detail test", false).await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "detailrepo");
    let dest = server.data_dir.path().join("clone-detail");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "hello.rs", "fn hello() {}", "feat: hello",
        "cursor", Some("gpt-4o"), Some("Create hello function"),
        Some("detail-sess-1"), None,
    );
    create_commit_with_trailers(
        &dest, "world.rs", "fn world() {}", "feat: world",
        "cursor", Some("gpt-4o"), Some("Create world function"),
        Some("detail-sess-1"), None,
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/bob/detailrepo/ai/detail-sess-1").await;
    let body = strip_hydration_markers(&resp.text().await.unwrap());

    assert!(body.contains("2 commits"), "Expected 2 commits in session");
    assert!(body.contains("cursor"), "Expected tool badge");
    assert!(body.contains("Create hello function"), "Expected first prompt");
    assert!(body.contains("Create world function"), "Expected second prompt");
}

/// Test that non-owners don't see operation buttons.
#[tokio::test]
async fn test_session_ops_hidden_for_non_owner() {
    let secret = "whsec_session_nonowner_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    // Owner creates repo and pushes AI commits
    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("privops", "Private ops test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "privops");
    let dest = server.data_dir.path().join("clone-privops");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "code.rs", "fn code() {}", "feat",
        "claude-code", None, Some("Write code"),
        Some("priv-sess-1"), None,
    );
    git_push(&dest);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Login as different user — also needs pro plan to access AI session detail
    client.register("eve", "eve@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "2", secret).await;
    client.login("eve", "password123").await;

    let resp = client.get("/alice/privops/ai/priv-sess-1").await;
    let body = resp.text().await.unwrap();

    // Non-owner should NOT see operation buttons
    assert!(!body.contains("Session Operations"), "Non-owner should not see operations panel");
    assert!(!body.contains("Revert Session"), "Non-owner should not see revert");
}
