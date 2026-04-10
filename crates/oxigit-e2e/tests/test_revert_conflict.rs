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

/// Extract redirect location from a Leptos server function response.
/// Leptos may return 200 or 3xx with a Location header.
fn get_redirect_location(resp: &reqwest::Response) -> Option<String> {
    resp.headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Test that reverting a session with conflicting subsequent changes
/// redirects to the conflict resolution page instead of failing.
#[tokio::test]
async fn test_revert_session_conflict_redirects() {
    let secret = "whsec_revert_conflict_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client
        .create_repo("conflictrepo", "Revert conflict test", false)
        .await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "conflictrepo",
    );
    let dest = server.data_dir.path().join("clone-conflict");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Step 1: Create a base commit
    create_commit(&dest, "shared.txt", "base content\n", "initial");

    // Step 2: Create an AI session commit that modifies shared.txt
    create_commit_with_trailers(
        &dest,
        "shared.txt",
        "ai modified content\n",
        "ai change",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Modify shared file"),
        Some("conflict-sess-1"),
        Some(1),
    );

    // Step 3: Create a non-AI commit that also modifies shared.txt (causes conflict on revert)
    create_commit(
        &dest,
        "shared.txt",
        "conflicting content that overlaps\n",
        "manual change",
    );

    let push = git_push(&dest);
    assert!(push.status.success(), "push should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Step 4: Attempt to revert the AI session — should detect conflict
    let resp = client
        .revert_session("alice", "conflictrepo", "conflict-sess-1")
        .await;

    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "Expected success or redirect, got status {}",
        status
    );

    let location =
        get_redirect_location(&resp).expect("Expected Location header on revert conflict response");

    assert!(
        location.contains("/alice/conflictrepo/conflicts/"),
        "Expected redirect to conflict resolution page, got: {}",
        location
    );

    // Step 5: Verify the conflict resolution page is accessible and shows revert info
    let conflict_page = client.get(&location).await;
    let body = strip_hydration_markers(&conflict_page.text().await.unwrap());

    assert!(
        body.contains("revert"),
        "Expected revert operation type in page body"
    );
    assert!(
        body.contains("shared.txt"),
        "Expected conflicting file name in page"
    );
    assert!(
        body.contains("Complete Revert"),
        "Expected Complete Revert button"
    );
}

/// Test that reverting a session without conflicts still works normally.
#[tokio::test]
async fn test_revert_session_clean_succeeds() {
    let secret = "whsec_revert_clean_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("bob", "password123").await;
    client
        .create_repo("cleanrepo", "Clean revert test", false)
        .await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "cleanrepo");
    let dest = server.data_dir.path().join("clone-clean");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Create base commit
    create_commit(&dest, "base.txt", "base", "initial");

    // Create AI session commits on separate files (no conflict possible)
    create_commit_with_trailers(
        &dest,
        "ai_file.rs",
        "fn ai() {}",
        "ai step 1",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add AI function"),
        Some("clean-sess-1"),
        Some(1),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Revert should succeed without conflicts — redirect back to session detail
    let resp = client
        .revert_session("bob", "cleanrepo", "clean-sess-1")
        .await;

    let status = resp.status();
    let location = get_redirect_location(&resp).unwrap_or_default();
    let body = resp.text().await.unwrap_or_default();

    assert!(
        status.is_success() || status.is_redirection(),
        "Expected success or redirect, got status {}; location={}; body={}",
        status,
        location,
        &body[..500.min(body.len())]
    );

    assert!(
        location.contains("/bob/cleanrepo/ai/clean-sess-1"),
        "Expected redirect back to session detail, got: {}; body={}",
        location,
        &body[..500.min(body.len())]
    );
}
