mod harness;

use harness::*;
use std::time::Duration;

const STRIPE_SECRET: &str = "whsec_provider_pipeline_test";

/// Helper: upgrade a user to Pro via Stripe webhook.
async fn upgrade_to_pro(client: &TestClient, base_url: &str, user_id: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_pp_{}","subscription":"sub_pp_{}","metadata":{{"plan":"flat"}}}}}}}}"#,
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
    assert!(status == 200 || status == 404, "Stripe webhook returned unexpected status: {status}");
}

/// Helper: full setup — register, upgrade to Pro, login, create repo, push initial commit.
/// Returns (client, clone_dest_path).
async fn setup_pro_user_with_repo(
    server: &TestServer,
    username: &str,
    repo_name: &str,
    user_id: &str,
) -> (TestClient, std::path::PathBuf) {
    let client = server.client();
    client
        .register(username, &format!("{username}@test.com"), "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), user_id).await;
    client.login(username, "password123").await;
    client.create_repo(repo_name, "pipeline test", false).await;

    let clone_url = http_clone_url(
        &server.base_url,
        username,
        "password123",
        username,
        repo_name,
    );
    let dest = server
        .data_dir
        .path()
        .join(format!("clone-{username}-{repo_name}"));
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);
    create_commit(&dest, "README.md", "# pipeline test", "initial commit");
    let push = git_push(&dest);
    assert!(push.status.success(), "initial push failed");

    (client, dest)
}

// ---------------------------------------------------------------------------
// Claude Code full pipeline via HTTP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_claude_code_pipeline_http() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "alice", "ccpipe", "1").await;

    // Install hooks
    let resp = client.install_ai_hook("alice", "ccpipe", "claude-code").await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "hook install failed"
    );

    // Pull hook files before making AI commits
    git_pull(&dest);

    // Create 2 commits with Claude Code trailers, different prompts
    create_commit_with_trailers(
        &dest,
        "auth.rs",
        "pub fn login() { /* auth */ }",
        "feat: add auth module",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Build the auth module"),
        Some("cc-session-1"),
        Some(1),
    );
    let sha1 = get_head_sha(&dest);

    create_commit_with_trailers(
        &dest,
        "auth_test.rs",
        "#[test] fn test_login() {}",
        "test: add auth tests",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add tests for auth"),
        Some("cc-session-1"),
        Some(2),
    );
    let sha2 = get_head_sha(&dest);

    let push = git_push(&dest);
    assert!(
        push.status.success(),
        "push failed: {}",
        String::from_utf8_lossy(&push.stderr)
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify commit diff shows AI metadata
    let resp = client.fetch_commit_diff("alice", "ccpipe", &sha1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("claude-code"), "Commit 1 should show claude-code tool");
    assert!(body.contains("cc-session-1"), "Commit 1 should show session ID");

    let resp = client.fetch_commit_diff("alice", "ccpipe", &sha2).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("claude-code"), "Commit 2 should show claude-code tool");

    // Verify AI timeline contains session
    let resp = client.get(&format!("/{}/{}/ai", "alice", "ccpipe")).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("claude-code"), "Timeline should show claude-code");
    assert!(body.contains("cc-session-1"), "Timeline should show session ID");
}

// ---------------------------------------------------------------------------
// Codex full pipeline via HTTP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_codex_pipeline_http() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "bob", "codexpipe", "1").await;

    // Install hooks
    client.install_ai_hook("bob", "codexpipe", "codex").await;
    git_pull(&dest);

    // Create commits with Codex trailers
    create_commit_with_trailers(
        &dest,
        "api.rs",
        "pub fn endpoint() {}",
        "feat: add API endpoint",
        "codex",
        Some("o3"),
        Some("Create REST endpoint"),
        Some("codex-session-1"),
        Some(1),
    );
    let sha1 = get_head_sha(&dest);

    create_commit_with_trailers(
        &dest,
        "api_test.rs",
        "#[test] fn test_api() {}",
        "test: add API tests",
        "codex",
        Some("o3"),
        Some("Write tests for API"),
        Some("codex-session-1"),
        Some(2),
    );

    let push = git_push(&dest);
    assert!(
        push.status.success(),
        "push failed: {}",
        String::from_utf8_lossy(&push.stderr)
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify
    let resp = client.fetch_commit_diff("bob", "codexpipe", &sha1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("codex"), "Commit should show codex tool");
    assert!(body.contains("codex-session-1"), "Commit should show session");

    let resp = client.get(&format!("/{}/{}/ai", "bob", "codexpipe")).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("codex"), "Timeline should show codex");
    assert!(body.contains("codex-session-1"), "Timeline should show session");
}

// ---------------------------------------------------------------------------
// Gemini full pipeline via HTTP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_gemini_pipeline_http() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "carol", "gempipe", "1").await;

    // Install hooks
    client.install_ai_hook("carol", "gempipe", "gemini-cli").await;
    git_pull(&dest);

    // Gemini has no model
    create_commit_with_trailers(
        &dest,
        "app.rs",
        "fn main() { println!(\"gemini\"); }",
        "feat: add app via Gemini",
        "gemini-cli",
        None, // no model
        Some("Create a hello world app"),
        Some("gemini-session-1"),
        Some(1),
    );
    let sha = get_head_sha(&dest);

    let push = git_push(&dest);
    assert!(
        push.status.success(),
        "push failed: {}",
        String::from_utf8_lossy(&push.stderr)
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    let resp = client.fetch_commit_diff("carol", "gempipe", &sha).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("gemini-cli"), "Commit should show gemini-cli tool");
    assert!(
        body.contains("gemini-session-1"),
        "Commit should show session"
    );

    let resp = client.get(&format!("/{}/{}/ai", "carol", "gempipe")).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("gemini-cli"), "Timeline should show gemini-cli");
}

// ---------------------------------------------------------------------------
// Claude Code pipeline via SSH
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_claude_code_pipeline_ssh() {
    if !ssh_available() {
        eprintln!("skipping SSH test: ssh-keygen not available");
        return;
    }

    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();
    client
        .register("dssh", "dssh@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("dssh", "password123").await;
    client.create_repo("ccpipessh", "ssh test", false).await;

    // Initial commit via HTTP (needed for repo init)
    let clone_url = http_clone_url(&server.base_url, "dssh", "password123", "dssh", "ccpipessh");
    let http_dest = server.data_dir.path().join("clone-dssh-http");
    git_clone_http(&clone_url, &http_dest);
    init_repo_config(&http_dest);
    create_commit(&http_dest, "README.md", "# ssh test", "initial");
    git_push(&http_dest);

    // Install hooks
    client
        .install_ai_hook("dssh", "ccpipessh", "claude-code")
        .await;

    // Set up SSH key and clone via SSH
    let (key_path, pub_key) = generate_ssh_keypair(server.data_dir.path());
    client.add_ssh_key("sshkey", &pub_key).await;

    let ssh_dest = server.data_dir.path().join("clone-dssh-ssh");
    let clone_result = git_clone_ssh(
        server.ssh_port,
        "dssh",
        "ccpipessh",
        &ssh_dest,
        &key_path,
    );
    assert!(
        clone_result.status.success(),
        "SSH clone failed: {}",
        String::from_utf8_lossy(&clone_result.stderr)
    );
    init_repo_config(&ssh_dest);

    // Create AI commit and push via SSH
    create_commit_with_trailers(
        &ssh_dest,
        "ssh_feature.rs",
        "pub fn ssh_thing() {}",
        "feat: SSH feature",
        "claude-code",
        Some("claude-sonnet-4-6"),
        Some("Add SSH feature"),
        Some("cc-ssh-session"),
        Some(1),
    );
    let sha = get_head_sha(&ssh_dest);

    let push = git_push_ssh(&ssh_dest, server.ssh_port, &key_path);
    assert!(
        push.status.success(),
        "SSH push failed: {}",
        String::from_utf8_lossy(&push.stderr)
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify
    let resp = client
        .fetch_commit_diff("dssh", "ccpipessh", &sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "SSH push should produce AI metadata for claude-code"
    );
    assert!(
        body.contains("cc-ssh-session"),
        "SSH push should preserve session ID"
    );
}

// ---------------------------------------------------------------------------
// Codex pipeline via SSH
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_codex_pipeline_ssh() {
    if !ssh_available() {
        eprintln!("skipping SSH test: ssh-keygen not available");
        return;
    }

    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();
    client
        .register("essh", "essh@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("essh", "password123").await;
    client
        .create_repo("codexpipessh", "ssh codex test", false)
        .await;

    // Initial commit via HTTP
    let clone_url =
        http_clone_url(&server.base_url, "essh", "password123", "essh", "codexpipessh");
    let http_dest = server.data_dir.path().join("clone-essh-http");
    git_clone_http(&clone_url, &http_dest);
    init_repo_config(&http_dest);
    create_commit(&http_dest, "README.md", "# ssh codex", "initial");
    git_push(&http_dest);

    // Install hooks + SSH key
    client
        .install_ai_hook("essh", "codexpipessh", "codex")
        .await;
    let (key_path, pub_key) = generate_ssh_keypair(server.data_dir.path());
    client.add_ssh_key("sshkey2", &pub_key).await;

    let ssh_dest = server.data_dir.path().join("clone-essh-ssh");
    let clone_result = git_clone_ssh(
        server.ssh_port,
        "essh",
        "codexpipessh",
        &ssh_dest,
        &key_path,
    );
    assert!(clone_result.status.success(), "SSH clone failed");
    init_repo_config(&ssh_dest);

    create_commit_with_trailers(
        &ssh_dest,
        "codex_feat.rs",
        "pub fn codex_ssh() {}",
        "feat: codex SSH feature",
        "codex",
        Some("o3"),
        Some("Build codex feature via SSH"),
        Some("codex-ssh-session"),
        Some(1),
    );
    let sha = get_head_sha(&ssh_dest);

    let push = git_push_ssh(&ssh_dest, server.ssh_port, &key_path);
    assert!(push.status.success(), "SSH push failed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let resp = client
        .fetch_commit_diff("essh", "codexpipessh", &sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("codex"), "SSH push should show codex tool");
    assert!(
        body.contains("codex-ssh-session"),
        "SSH push should preserve session"
    );
}

// ---------------------------------------------------------------------------
// Gemini pipeline via SSH
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_gemini_pipeline_ssh() {
    if !ssh_available() {
        eprintln!("skipping SSH test: ssh-keygen not available");
        return;
    }

    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let client = server.client();
    client
        .register("fssh", "fssh@test.com", "password123")
        .await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1").await;
    client.login("fssh", "password123").await;
    client
        .create_repo("gempipessh", "ssh gemini test", false)
        .await;

    // Initial commit via HTTP
    let clone_url =
        http_clone_url(&server.base_url, "fssh", "password123", "fssh", "gempipessh");
    let http_dest = server.data_dir.path().join("clone-fssh-http");
    git_clone_http(&clone_url, &http_dest);
    init_repo_config(&http_dest);
    create_commit(&http_dest, "README.md", "# ssh gemini", "initial");
    git_push(&http_dest);

    // Install hooks + SSH key
    client
        .install_ai_hook("fssh", "gempipessh", "gemini-cli")
        .await;
    let (key_path, pub_key) = generate_ssh_keypair(server.data_dir.path());
    client.add_ssh_key("sshkey3", &pub_key).await;

    let ssh_dest = server.data_dir.path().join("clone-fssh-ssh");
    let clone_result = git_clone_ssh(
        server.ssh_port,
        "fssh",
        "gempipessh",
        &ssh_dest,
        &key_path,
    );
    assert!(clone_result.status.success(), "SSH clone failed");
    init_repo_config(&ssh_dest);

    create_commit_with_trailers(
        &ssh_dest,
        "gemini_feat.rs",
        "pub fn gemini_ssh() {}",
        "feat: gemini SSH feature",
        "gemini-cli",
        None,
        Some("Build gemini feature via SSH"),
        Some("gemini-ssh-session"),
        Some(1),
    );
    let sha = get_head_sha(&ssh_dest);

    let push = git_push_ssh(&ssh_dest, server.ssh_port, &key_path);
    assert!(push.status.success(), "SSH push failed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let resp = client
        .fetch_commit_diff("fssh", "gempipessh", &sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("gemini-cli"),
        "SSH push should show gemini-cli tool"
    );
    assert!(
        body.contains("gemini-ssh-session"),
        "SSH push should preserve session"
    );
}

// ---------------------------------------------------------------------------
// Cross-provider: all 3 tools in one repo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cross_provider_same_repo() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "multi", "crossrepo", "1").await;

    // Install all 3 hooks
    for tool in &["claude-code", "codex", "gemini-cli"] {
        client
            .install_ai_hook("multi", "crossrepo", tool)
            .await;
    }
    git_pull(&dest);

    // Claude Code commit
    create_commit_with_trailers(
        &dest,
        "claude_file.rs",
        "// claude",
        "feat: claude feature",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Build with Claude"),
        Some("cross-claude-session"),
        Some(1),
    );

    // Codex commit
    create_commit_with_trailers(
        &dest,
        "codex_file.rs",
        "// codex",
        "feat: codex feature",
        "codex",
        Some("o3"),
        Some("Build with Codex"),
        Some("cross-codex-session"),
        Some(1),
    );

    // Gemini commit
    create_commit_with_trailers(
        &dest,
        "gemini_file.rs",
        "// gemini",
        "feat: gemini feature",
        "gemini-cli",
        None,
        Some("Build with Gemini"),
        Some("cross-gemini-session"),
        Some(1),
    );

    let push = git_push(&dest);
    assert!(
        push.status.success(),
        "push failed: {}",
        String::from_utf8_lossy(&push.stderr)
    );
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify AI timeline contains all 3 providers
    let resp = client.get(&format!("/{}/{}/ai", "multi", "crossrepo")).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "Timeline should contain claude-code"
    );
    assert!(body.contains("codex"), "Timeline should contain codex");
    assert!(
        body.contains("gemini-cli"),
        "Timeline should contain gemini-cli"
    );

    // Verify all 3 sessions appear
    assert!(
        body.contains("cross-claude-session"),
        "Timeline should contain Claude session"
    );
    assert!(
        body.contains("cross-codex-session"),
        "Timeline should contain Codex session"
    );
    assert!(
        body.contains("cross-gemini-session"),
        "Timeline should contain Gemini session"
    );
}

// ---------------------------------------------------------------------------
// Mixed AI and regular commits
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_mixed_ai_and_regular_commits() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "mixuser", "mixrepo", "1").await;

    // Regular commit (no trailers)
    create_commit(&dest, "normal.txt", "regular content", "chore: regular commit");
    let regular_sha = get_head_sha(&dest);

    // AI commit
    create_commit_with_trailers(
        &dest,
        "ai_file.rs",
        "// ai generated",
        "feat: AI feature",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Generate feature"),
        Some("mix-session"),
        Some(1),
    );
    let ai_sha = get_head_sha(&dest);

    // Another regular commit
    create_commit(&dest, "normal2.txt", "more regular", "docs: update docs");

    let push = git_push(&dest);
    assert!(push.status.success(), "push failed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // AI timeline should contain the AI commit but not the regular ones
    let resp = client.get(&format!("/{}/{}/ai", "mixuser", "mixrepo")).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "Timeline should show AI commit"
    );
    assert!(
        body.contains("mix-session"),
        "Timeline should show AI session"
    );

    // Verify the regular commit doesn't have AI metadata
    let resp = client
        .fetch_commit_diff("mixuser", "mixrepo", &regular_sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("claude-code") || body.contains(&regular_sha),
        "Regular commit should not have AI tool attribution"
    );

    // Verify the AI commit does have metadata
    let resp = client
        .fetch_commit_diff("mixuser", "mixrepo", &ai_sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "AI commit should have tool attribution"
    );
}

// ---------------------------------------------------------------------------
// Multi-prompt session per provider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_prompt_session_per_provider() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "promptu", "promptrepo", "1").await;

    // Claude Code session with 3 prompts, prompt 2 has 2 commits
    create_commit_with_trailers(
        &dest,
        "p1.rs",
        "// prompt 1",
        "feat: prompt 1 work",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("First prompt"),
        Some("multi-prompt-session"),
        Some(1),
    );

    create_commit_with_trailers(
        &dest,
        "p2a.rs",
        "// prompt 2 part a",
        "feat: prompt 2 first file",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Second prompt"),
        Some("multi-prompt-session"),
        Some(2),
    );

    create_commit_with_trailers(
        &dest,
        "p2b.rs",
        "// prompt 2 part b",
        "feat: prompt 2 second file",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Second prompt"),
        Some("multi-prompt-session"),
        Some(2),
    );

    create_commit_with_trailers(
        &dest,
        "p3.rs",
        "// prompt 3",
        "feat: prompt 3 work",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Third prompt"),
        Some("multi-prompt-session"),
        Some(3),
    );

    let push = git_push(&dest);
    assert!(push.status.success(), "push failed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify AI timeline shows the session
    let resp = client.get(&format!("/{}/{}/ai", "promptu", "promptrepo")).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("multi-prompt-session"),
        "Timeline should show session"
    );
    assert!(
        body.contains("claude-code"),
        "Timeline should show tool"
    );

    // Verify all commits appear (4 AI commits total)
    let resp = client.fetch_commits("promptu", "promptrepo").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("prompt 1 work"),
        "Commit list should contain prompt 1 commit"
    );
    assert!(
        body.contains("prompt 2 first file"),
        "Commit list should contain prompt 2 first commit"
    );
    assert!(
        body.contains("prompt 2 second file"),
        "Commit list should contain prompt 2 second commit"
    );
    assert!(
        body.contains("prompt 3 work"),
        "Commit list should contain prompt 3 commit"
    );
}

// ---------------------------------------------------------------------------
// Legacy context.json and trailers coexist
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_legacy_context_json_and_trailers_coexist() {
    let server = TestServer::start_with_stripe(STRIPE_SECRET).await;
    let (client, dest) = setup_pro_user_with_repo(&server, "legacy", "legacyrepo", "1").await;

    // Legacy commit with .oxigit/context.json
    create_commit_with_oxigit_context(
        &dest,
        "legacy_file.rs",
        "// legacy",
        "feat: legacy AI commit",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Legacy prompt"),
        Some("legacy-session"),
    );
    let legacy_sha = get_head_sha(&dest);

    // New-style commit with trailers
    create_commit_with_trailers(
        &dest,
        "new_file.rs",
        "// new style",
        "feat: new style AI commit",
        "codex",
        Some("o3"),
        Some("New style prompt"),
        Some("new-session"),
        Some(1),
    );
    let new_sha = get_head_sha(&dest);

    let push = git_push(&dest);
    assert!(push.status.success(), "push failed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Both should appear in timeline
    let resp = client.get(&format!("/{}/{}/ai", "legacy", "legacyrepo")).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "Timeline should contain legacy claude-code commit"
    );
    assert!(
        body.contains("codex"),
        "Timeline should contain new codex commit"
    );

    // Verify individual commits
    let resp = client
        .fetch_commit_diff("legacy", "legacyrepo", &legacy_sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "Legacy commit should show claude-code"
    );

    let resp = client
        .fetch_commit_diff("legacy", "legacyrepo", &new_sha)
        .await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("codex"), "New commit should show codex");
}
