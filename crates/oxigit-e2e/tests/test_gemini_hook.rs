mod harness;

use harness::*;

const STRIPE_SECRET: &str = "whsec_gemini_test";

/// Helper: upgrade a user to Pro via Stripe webhook.
async fn upgrade_to_pro(client: &TestClient, base_url: &str, user_id: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_gem_{}","subscription":"sub_gem_{}","metadata":{{"plan":"flat"}}}}}}}}"#,
        user_id, user_id, user_id
    );
    let timestamp = "1234567890";
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(STRIPE_SECRET.as_bytes()).unwrap();
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
    assert_eq!(resp.status().as_u16(), 200, "Flat upgrade webhook should succeed");
}

/// Test that installing the Gemini hook produces correct script content:
/// - Session capture script must output JSON to stdout (Gemini CLI requirement)
/// - Config timeout must be in milliseconds (5000, not 5)
#[tokio::test]
async fn test_gemini_hook_script_content() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("gem", "gem@test.com", "password123").await;
    client.login("gem", "password123").await;
    client.create_repo("gemrepo", "Gemini hook test", false).await;

    // Need an initial commit so the repo has a branch
    let clone_url = http_clone_url(&server.base_url, "gem", "password123", "gem", "gemrepo");
    let dest = server.data_dir.path().join("clone-gem-init");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);
    create_commit(&dest, "README.md", "# Gemini test", "initial commit");
    let push_result = git_push(&dest);
    assert!(push_result.status.success(), "initial push failed");

    // Install Gemini hook
    let resp = client.install_ai_hook("gem", "gemrepo", "gemini-cli").await;
    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "install_ai_hook failed with status: {}",
        status
    );

    // Fetch the session capture script and verify it outputs JSON
    // get_blob returns a BlobResponse with highlighted_html containing the file content
    let resp = client
        .get_blob("gem", "gemrepo", ".gemini/hooks/oxigit-session.sh")
        .await;
    let script_body = resp.text().await.unwrap();
    assert!(
        script_body.contains("decision") && script_body.contains("allow"),
        "Session capture script must output JSON decision:allow for Gemini CLI, got: {}",
        &script_body[..500.min(script_body.len())]
    );

    // Fetch the settings.json and verify timeout is in milliseconds
    let resp = client
        .get_blob("gem", "gemrepo", ".gemini/settings.json")
        .await;
    let config_body = resp.text().await.unwrap();
    assert!(
        config_body.contains("5000"),
        "Config timeout must be 5000ms, got: {}",
        &config_body[..300.min(config_body.len())]
    );

    // Verify the universal prepare-commit-msg hook was also installed
    let resp = client
        .get_blob("gem", "gemrepo", ".githooks/prepare-commit-msg")
        .await;
    let hook_body = resp.text().await.unwrap();
    assert!(
        hook_body.contains("Oxigit universal prepare-commit-msg hook"),
        "Universal hook must be installed"
    );
}

/// Test that the full Gemini flow works end-to-end:
/// Install hook, simulate Gemini session, push, verify AI activity.
#[tokio::test]
async fn test_gemini_trailers_produce_ai_activity() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();

    client
        .register("gemi", "gemi@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("gemi", "password123").await;
    client
        .create_repo("gemflow", "Gemini flow test", false)
        .await;

    let clone_url =
        http_clone_url(&server.base_url, "gemi", "password123", "gemi", "gemflow");
    let dest = server.data_dir.path().join("clone-gemflow");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Simulate what the Gemini hook would produce: a commit with Oxigit trailers
    create_commit_with_trailers(
        &dest,
        "app.rs",
        "fn main() { println!(\"gemini\"); }",
        "feat: add app via Gemini CLI",
        "gemini-cli",
        None, // Gemini has no model field
        Some("Create a hello world app"),
        Some("gemini-session-42"),
        None,
    );
    let sha = get_head_sha(&dest);
    let push_result = git_push(&dest);
    assert!(
        push_result.status.success(),
        "push failed: {}",
        String::from_utf8_lossy(&push_result.stderr)
    );

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify AI metadata was captured
    let resp = client.fetch_commit_diff("gemi", "gemflow", &sha).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("gemini-cli"),
        "Expected gemini-cli AI tool in commit detail, got: {}",
        &body[..300.min(body.len())]
    );
    assert!(
        body.contains("gemini-session-42"),
        "Expected Gemini session ID in commit detail"
    );
}
