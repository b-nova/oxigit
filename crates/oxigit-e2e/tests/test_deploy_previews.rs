mod harness;

use harness::*;

/// Helper: simulate upgrading a user to Pro (Flat) via Stripe webhook.
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
    assert_eq!(resp.status().as_u16(), 200, "webhook should succeed");
}

/// Test: add a webhook and verify it appears in the list.
#[tokio::test]
async fn add_and_list_webhooks() {
    let secret = "whsec_webhook_add_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Add a webhook
    client
        .add_webhook("alice", "myrepo", "https://example.com/hook", "mysecret")
        .await;

    // List webhooks
    let resp = client.list_repo_webhooks("alice", "myrepo").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("example.com/hook"),
        "webhook should appear in list: {body}"
    );
}

/// Test: delete a webhook and verify it is removed.
#[tokio::test]
async fn delete_webhook() {
    let secret = "whsec_webhook_del_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Add webhook
    client
        .add_webhook("alice", "myrepo", "https://example.com/hook", "")
        .await;

    // List to confirm and extract ID (webhook_id is typically 1 for first webhook)
    let resp = client.list_repo_webhooks("alice", "myrepo").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("example.com/hook"), "webhook should exist before delete");

    // Delete webhook (first webhook ID is 1)
    client.delete_webhook("alice", "myrepo", 1).await;

    // List again — webhook should be gone
    let resp = client.list_repo_webhooks("alice", "myrepo").await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("example.com/hook"),
        "webhook should be removed after delete: {body}"
    );
}

/// Test: non-owner cannot list another user's webhooks.
#[tokio::test]
async fn webhook_requires_repo_owner() {
    let secret = "whsec_webhook_owner_test";
    let server = TestServer::start_with_stripe(secret).await;

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&alice, &alice.base_url.clone(), "1", secret).await;
    alice.login("alice", "password123").await;
    alice.create_repo("myrepo", "test repo", false).await;
    alice
        .add_webhook("alice", "myrepo", "https://example.com/hook", "")
        .await;

    // Bob tries to list Alice's webhooks
    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    let resp = bob.list_repo_webhooks("alice", "myrepo").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("owner") || body.contains("Not") || body.contains("error"),
        "non-owner should be denied access to webhooks: {body}"
    );
}

/// Test: deploy callback updates preview status and URL for a Flat plan user.
#[tokio::test]
async fn deploy_callback_updates_preview() {
    let secret = "whsec_deploy_callback_test";
    let server = TestServer::start_with_stripe(secret).await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("webapp", "test", false).await;

    // Add a webhook so deploy previews get created on push
    client
        .add_webhook("alice", "webapp", "https://example.com/deploy", "")
        .await;

    // Push a commit
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "webapp");
    let repo_dir = tmp.path().join("webapp");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "index.html", "<h1>Hello</h1>\n", "initial commit");
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    // Wait for post-receive hook to process and create deploy preview record
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Send deploy callback to update the preview
    let resp = client
        .client
        .post(format!("{}/api/deploy-callback/{}", client.base_url, sha))
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "repo_owner": "alice",
            "repo_name": "webapp",
            "status": "ready",
            "preview_url": "https://preview.example.com/abc"
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap();
    assert_eq!(
        status, 200,
        "deploy callback should succeed for Flat user, got {status}: {body}"
    );
}

/// Test: deploy callback is blocked for free plan users.
#[tokio::test]
async fn deploy_callback_blocked_for_free_plan() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("webapp", "test", false).await;

    // Push a commit (no webhook, so no deploy preview record — but callback should fail on plan check first)
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "webapp");
    let repo_dir = tmp.path().join("webapp");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "index.html", "<h1>Hello</h1>\n", "initial");
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    let resp = client
        .client
        .post(format!("{}/api/deploy-callback/{}", client.base_url, sha))
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "repo_owner": "alice",
            "repo_name": "webapp",
            "status": "ready",
            "preview_url": "https://preview.example.com/abc"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        403,
        "deploy callback should return 403 for free plan user"
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Pro or higher plan"),
        "should mention plan requirement: {body}"
    );
}

/// Test: deploy preview URL is shown on the commit view page.
#[tokio::test]
async fn deploy_preview_shown_on_commit_view() {
    let secret = "whsec_deploy_view_test";
    let server = TestServer::start_with_stripe(secret).await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("webapp", "test", false).await;

    // Add webhook so deploy preview record gets created
    client
        .add_webhook("alice", "webapp", "https://example.com/deploy", "")
        .await;

    // Push a commit
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "webapp");
    let repo_dir = tmp.path().join("webapp");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "index.html", "<h1>Hello</h1>\n", "initial");
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    // Wait for post-receive hook to create deploy preview record
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Send deploy callback with preview URL
    client
        .client
        .post(format!("{}/api/deploy-callback/{}", client.base_url, sha))
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "repo_owner": "alice",
            "repo_name": "webapp",
            "status": "ready",
            "preview_url": "https://preview.example.com/xyz"
        }))
        .send()
        .await
        .unwrap();

    // Fetch commit view — should show preview URL
    let resp = client.fetch_commit_diff("alice", "webapp", &sha).await;
    let body = resp.text().await.unwrap();
    let body = strip_hydration_markers(&body);

    assert!(
        body.contains("preview.example.com") || body.contains("Live Preview"),
        "commit view should show deploy preview URL or link: {body}"
    );
}
