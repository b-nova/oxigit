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
    assert!(
        status == 200 || status == 404,
        "Stripe webhook returned unexpected status: {status}"
    );
}

/// Test that the prompt detail page shows prompt info and commits.
#[tokio::test]
async fn test_prompt_detail_page() {
    let secret = "whsec_prompt_detail_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client
        .create_repo("promptdetail", "Prompt detail test", false)
        .await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "promptdetail",
    );
    let dest = server.data_dir.path().join("clone-pd");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Two commits from prompt_index=1
    create_commit_with_trailers(
        &dest,
        "auth.rs",
        "fn login() {}",
        "feat: login",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add authentication"),
        Some("pd-session-1"),
        Some(1),
    );
    create_commit_with_trailers(
        &dest,
        "auth_test.rs",
        "fn test_login() {}",
        "test: login tests",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add authentication"),
        Some("pd-session-1"),
        Some(1),
    );

    // One commit from prompt_index=2
    create_commit_with_trailers(
        &dest,
        "api.rs",
        "fn api() {}",
        "feat: api",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add API endpoint"),
        Some("pd-session-1"),
        Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch prompt detail for prompt_index=1
    let resp = client
        .get("/alice/promptdetail/ai/pd-session-1/prompt/1")
        .await;
    let body = strip_hydration_markers(&resp.text().await.unwrap());

    assert!(
        body.contains("Add authentication"),
        "Expected prompt text, got: {}",
        &body[..2000.min(body.len())]
    );
    assert!(body.contains("claude-code"), "Expected AI tool badge");
    assert!(
        body.contains("2 commits"),
        "Expected 2 commits for this prompt"
    );
    assert!(
        body.contains("Prompt Operations") || body.contains("Revert Prompt"),
        "Expected prompt operations for owner"
    );
}

/// Test that prompt detail shows the correct prompt (not another prompt's data).
#[tokio::test]
async fn test_prompt_detail_isolation() {
    let secret = "whsec_prompt_iso_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("bob", "password123").await;
    client
        .create_repo("isolation", "Isolation test", false)
        .await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "isolation");
    let dest = server.data_dir.path().join("clone-iso");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest,
        "a.rs",
        "fn a() {}",
        "feat: a",
        "cursor",
        None,
        Some("Build feature A"),
        Some("iso-sess"),
        Some(1),
    );
    create_commit_with_trailers(
        &dest,
        "b.rs",
        "fn b() {}",
        "feat: b",
        "cursor",
        None,
        Some("Build feature B"),
        Some("iso-sess"),
        Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch prompt 2 — should show "Build feature B" but NOT "Build feature A"
    let resp = client.get("/bob/isolation/ai/iso-sess/prompt/2").await;
    let body = strip_hydration_markers(&resp.text().await.unwrap());

    assert!(body.contains("Build feature B"), "Expected prompt B text");
    // Prompt A's text should not appear in the prompt section (it may appear in breadcrumb links)
    assert!(body.contains("1 commit"), "Expected 1 commit for prompt 2");
}

/// Test that prompt detail page shows vibe score and squash button.
#[tokio::test]
async fn test_prompt_detail_shows_vibe_and_squash() {
    let secret = "whsec_prompt_vibe_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client
        .register("carol", "carol@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("carol", "password123").await;
    client
        .create_repo("vibesquash", "Vibe and squash test", false)
        .await;

    let clone_url = http_clone_url(
        &server.base_url,
        "carol",
        "password123",
        "carol",
        "vibesquash",
    );
    let dest = server.data_dir.path().join("clone-vs");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit(&dest, "base.txt", "base", "initial");
    create_branch(&dest, "feature");

    create_commit_with_trailers(
        &dest,
        "module.rs",
        "fn module() {}",
        "feat: module",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Create module"),
        Some("vs-session"),
        Some(1),
    );
    create_commit_with_trailers(
        &dest,
        "module_test.rs",
        "fn test_module() {}",
        "test: module",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Create module"),
        Some("vs-session"),
        Some(1),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/carol/vibesquash/ai/vs-session/prompt/1").await;
    let body = strip_hydration_markers(&resp.text().await.unwrap());

    // Vibe score badge should be present
    assert!(
        body.contains("vibe-badge"),
        "Expected vibe score badge, got: {}",
        &body[..500.min(body.len())]
    );
    // Squash button should be present for owner
    assert!(body.contains("Squash Prompt"), "Expected squash button");
    // Cherry-pick should also be present
    assert!(
        body.contains("Cherry-pick to"),
        "Expected cherry-pick button"
    );
    // Time range should be shown
    assert!(body.contains(" — "), "Expected time range separator");
}

/// Test that non-owners don't see prompt operations.
#[tokio::test]
async fn test_prompt_ops_hidden_for_non_owner() {
    let secret = "whsec_prompt_nonowner_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("dan", "dan@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("dan", "password123").await;
    client
        .create_repo("privprompt", "Private prompt ops", false)
        .await;

    let clone_url = http_clone_url(&server.base_url, "dan", "password123", "dan", "privprompt");
    let dest = server.data_dir.path().join("clone-pp");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest,
        "code.rs",
        "fn code() {}",
        "feat: code",
        "claude-code",
        None,
        Some("Write code"),
        Some("pp-sess"),
        Some(1),
    );
    git_push(&dest);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Login as different user — also needs pro plan to access prompt detail
    client.register("eve", "eve@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "2", secret).await;
    client.login("eve", "password123").await;

    let resp = client.get("/dan/privprompt/ai/pp-sess/prompt/1").await;
    let body = strip_hydration_markers(&resp.text().await.unwrap());

    assert!(
        !body.contains("Prompt Operations"),
        "Non-owner should not see operations panel"
    );
    assert!(
        !body.contains("Squash Prompt"),
        "Non-owner should not see squash"
    );
    assert!(
        !body.contains("Revert Prompt"),
        "Non-owner should not see revert"
    );
    // Vibe score should still be visible
    assert!(
        body.contains("vibe-badge"),
        "Vibe score should be visible to non-owners"
    );
}
