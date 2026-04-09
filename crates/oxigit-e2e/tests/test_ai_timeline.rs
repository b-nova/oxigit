mod harness;

use harness::*;

const STRIPE_SECRET: &str = "whsec_ai_test";

/// Helper: upgrade a user to Pro via Stripe webhook.
/// In non-saas builds, users are already on the "flat" plan, so this is a no-op.
async fn upgrade_to_pro(client: &TestClient, base_url: &str, user_id: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_ai_{}","subscription":"sub_ai_{}","metadata":{{"plan":"flat"}}}}}}}}"#,
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
    let status = resp.status().as_u16();
    // In non-saas builds the webhook route doesn't exist (404) — users are already pro
    assert!(status == 200 || status == 404, "Stripe webhook returned unexpected status: {}", status);
}

/// Test that pushing a commit with .oxigit/context.json causes metadata to appear in the commit detail.
#[tokio::test]
async fn test_ai_context_on_http_push() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("alice", "password123").await;
    client.create_repo("airepo", "AI test repo", false).await;

    // Clone, create commit with .oxigit/context.json, push
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "airepo");
    let dest = server.data_dir.path().join("clone-ai");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_oxigit_context(
        &dest,
        "hello.rs",
        "fn main() { println!(\"hello\"); }",
        "feat: add hello function",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Create a hello world function in Rust"),
        Some("session-abc-123"),
    );
    let sha = get_head_sha(&dest);
    let push_result = git_push(&dest);
    assert!(push_result.status.success(), "push failed: {}", String::from_utf8_lossy(&push_result.stderr));

    // Give the background task a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch commit detail and verify AI metadata is present
    let resp = client.fetch_commit_diff("alice", "airepo", &sha).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("claude-code"), "Expected AI tool in commit detail, got: {}", &body[..200.min(body.len())]);
    assert!(body.contains("claude-opus-4-6"), "Expected AI model in commit detail");
}

/// Test that the commits list includes AI tool badges.
#[tokio::test]
async fn test_ai_badge_in_commits_list() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.login("bob", "password123").await;
    client.create_repo("ailist", "Test AI list", false).await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "ailist");
    let dest = server.data_dir.path().join("clone-ailist");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // One commit without AI context, one with
    create_commit(&dest, "file1.txt", "content", "regular commit");
    create_commit_with_oxigit_context(
        &dest,
        "file2.txt",
        "more content",
        "feat: AI generated file",
        "cursor",
        None,
        None,
        None,
    );
    let push_result = git_push(&dest);
    assert!(push_result.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch commits list and check for AI tool presence
    let resp = client.fetch_commits("bob", "ailist").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("cursor"), "Expected AI tool badge 'cursor' in commits list");
}

/// Test manual AI metadata attachment via server function.
#[tokio::test]
async fn test_manual_ai_metadata_attachment() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();

    client.register("carol", "carol@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("carol", "password123").await;
    client.create_repo("manual", "Manual metadata test", false).await;

    let clone_url = http_clone_url(&server.base_url, "carol", "password123", "carol", "manual");
    let dest = server.data_dir.path().join("clone-manual");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Push a regular commit (no AI context)
    create_commit(&dest, "app.rs", "fn app() {}", "initial commit");
    let sha = get_head_sha(&dest);
    let push_result = git_push(&dest);
    assert!(push_result.status.success());

    // Attach AI metadata manually
    let resp = client.attach_ai_metadata(
        "carol",
        "manual",
        &sha,
        "aider",
        Some("gpt-4o"),
        Some("Fix the app function"),
        Some("manual-session-1"),
        Some("app.rs"),
    ).await;
    // Should succeed (2xx or redirect)
    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "attach_ai_metadata failed with status: {}",
        status
    );

    // Verify metadata shows up in commit detail
    let resp = client.fetch_commit_diff("carol", "manual", &sha).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("aider"), "Expected manually attached AI tool in commit detail");
}

/// Test that .oxigit/context.json survives SSH push.
#[tokio::test]
async fn test_ai_context_via_ssh_push() {
    if !ssh_available() {
        eprintln!("Skipping SSH test — ssh-keygen not available");
        return;
    }

    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();

    client.register("dave", "dave@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("dave", "password123").await;
    client.create_repo("sshrepo", "SSH AI test", false).await;

    // Generate SSH key and register
    let (key_path, pub_key) = generate_ssh_keypair(server.data_dir.path());
    client.add_ssh_key("testkey", &pub_key).await;

    // Clone via SSH
    let dest = server.data_dir.path().join("clone-ssh-ai");
    let clone_result = git_clone_ssh(server.ssh_port, "dave", "sshrepo", &dest, &key_path);
    assert!(clone_result.status.success(), "SSH clone failed");
    init_repo_config(&dest);

    // Create commit with .oxigit/context.json and push via SSH
    create_commit_with_oxigit_context(
        &dest,
        "main.py",
        "print('hello')",
        "feat: add python script",
        "copilot",
        Some("gpt-4"),
        Some("Write a hello world in Python"),
        None,
    );
    let sha = get_head_sha(&dest);
    let push_result = git_push_ssh(&dest, server.ssh_port, &key_path);
    assert!(push_result.status.success(), "SSH push failed: {}", String::from_utf8_lossy(&push_result.stderr));

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify via commit detail API
    let resp = client.fetch_commit_diff("dave", "sshrepo", &sha).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("copilot"), "Expected AI tool from SSH push in commit detail");
}
