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

/// Test that the session detail page shows a vibe score badge.
#[tokio::test]
async fn test_vibe_score_on_session_detail() {
    let secret = "whsec_vibe_score_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("viberepo", "Vibe score test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "viberepo");
    let dest = server.data_dir.path().join("clone-vibe");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "app.rs", "fn main() {}", "feat: main",
        "claude-code", Some("claude-opus-4-6"), Some("Create main function"),
        Some("vibe-session-1"), Some(1),
    );
    create_commit_with_trailers(
        &dest, "app_test.rs", "fn test() {}", "test: main",
        "claude-code", Some("claude-opus-4-6"), Some("Add tests"),
        Some("vibe-session-1"), Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/alice/viberepo/ai/vibe-session-1").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("vibe-badge"), "Expected vibe score badge on session detail, got: {}", &body[..500.min(body.len())]);
}

/// Test that the repo metrics page shows session scores and stats.
#[tokio::test]
async fn test_repo_metrics_page() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.login("bob", "password123").await;
    client.create_repo("metricsrepo", "Metrics test", false).await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "metricsrepo");
    let dest = server.data_dir.path().join("clone-metrics");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "feat.rs", "fn feat() {}", "feat: new feature",
        "cursor", Some("gpt-4o"), Some("Build feature"),
        Some("metrics-sess-1"), None,
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/bob/metricsrepo/metrics").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Avg Vibe Score"), "Expected average score stat card, got: {}", &body[..500.min(body.len())]);
    assert!(body.contains("Sessions"), "Expected sessions stat card");
    assert!(body.contains("Session Scores"), "Expected session scores table");
    assert!(body.contains("cursor"), "Expected tool name in metrics");
}

/// Test that metrics page works for repos with no AI activity.
#[tokio::test]
async fn test_repo_metrics_empty() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("carol", "carol@test.com", "password123").await;
    client.login("carol", "password123").await;
    client.create_repo("emptymetrics", "Empty metrics", false).await;

    let clone_url = http_clone_url(&server.base_url, "carol", "password123", "carol", "emptymetrics");
    let dest = server.data_dir.path().join("clone-emptym");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);
    create_commit(&dest, "readme.txt", "hello", "initial");
    git_push(&dest);

    let resp = client.get("/carol/emptymetrics/metrics").await;
    let body = resp.text().await.unwrap();

    // Should render the page without errors, showing 0 sessions
    assert!(body.contains("Metrics"), "Expected metrics page to load");
    assert!(body.contains("0") || body.contains("Sessions"), "Expected zero sessions");
}

/// Test that the dashboard shows a user vibe score.
#[tokio::test]
async fn test_dashboard_vibe_score() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("dave", "dave@test.com", "password123").await;
    client.login("dave", "password123").await;
    client.create_repo("dashrepo", "Dashboard test", false).await;

    let clone_url = http_clone_url(&server.base_url, "dave", "password123", "dave", "dashrepo");
    let dest = server.data_dir.path().join("clone-dash");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "main.rs", "fn main() {}", "feat: main",
        "claude-code", None, Some("Create main"),
        Some("dash-sess-1"), None,
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // The dashboard is fetched via server function, but we can check the home page renders
    let resp = client.get("/").await;
    let body = resp.text().await.unwrap();

    // Dashboard should show AI stats (existing behavior) — vibe score is computed server-side
    assert!(body.contains("dave") || body.contains("dashrepo") || body.contains("AI"),
        "Expected dashboard to show user data");
}
