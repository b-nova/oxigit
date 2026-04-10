mod harness;

use harness::*;

// ---------------------------------------------------------------------------
// Hook installation tests for all three AI providers
// ---------------------------------------------------------------------------

/// Helper: set up a user with a repo that has an initial commit (needed for hook install).
async fn setup_user_with_repo(
    server: &TestServer,
    username: &str,
    repo_name: &str,
) -> (TestClient, std::path::PathBuf) {
    let client = server.client();
    client
        .register(username, &format!("{username}@test.com"), "password123")
        .await;
    client.login(username, "password123").await;
    client.create_repo(repo_name, "hook test", false).await;

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
    create_commit(&dest, "README.md", "# hook test", "initial commit");
    let push = git_push(&dest);
    assert!(push.status.success(), "initial push failed");

    (client, dest)
}

// ---------------------------------------------------------------------------
// Claude Code hooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_claude_code_hooks() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "alice", "ccrepo").await;

    let resp = client
        .install_ai_hook("alice", "ccrepo", "claude-code")
        .await;
    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "install_ai_hook failed: {status}"
    );

    // Verify the PreToolUse hook script
    let resp = client
        .get_blob("alice", "ccrepo", ".claude/hooks/oxigit-context.sh")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("oxigit-pending.json"),
        "Claude hook must write oxigit-pending.json, got: {}",
        &body[..500.min(body.len())]
    );

    // Verify the settings.json config
    let resp = client
        .get_blob("alice", "ccrepo", ".claude/settings.json")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("PreToolUse"),
        "Claude config must contain PreToolUse, got: {}",
        &body[..300.min(body.len())]
    );
    assert!(
        body.contains("Bash"),
        "Claude config must match on Bash tool, got: {}",
        &body[..300.min(body.len())]
    );

    // Claude Code does NOT use the universal prepare-commit-msg (it installs its own dynamically)
    let resp = client
        .get_blob("alice", "ccrepo", ".githooks/prepare-commit-msg")
        .await;
    let body = resp.text().await.unwrap();
    // The blob endpoint returns an error or empty if the file doesn't exist
    assert!(
        !body.contains("Oxigit universal prepare-commit-msg hook"),
        "Claude Code should NOT install the universal prepare-commit-msg hook"
    );
}

// ---------------------------------------------------------------------------
// Codex hooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_codex_hooks() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "bob", "codexrepo").await;

    let resp = client.install_ai_hook("bob", "codexrepo", "codex").await;
    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "install_ai_hook failed: {status}"
    );

    // Verify session capture script
    let resp = client
        .get_blob("bob", "codexrepo", ".codex/hooks/oxigit-session.sh")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("last-ai-session.json"),
        "Codex hook must write last-ai-session.json"
    );
    assert!(
        body.contains("decision") && body.contains("allow"),
        "Codex hook must output JSON decision:allow"
    );

    // Verify config with UserPromptSubmit event and ms timeout
    let resp = client
        .get_blob("bob", "codexrepo", ".codex/hooks.json")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("UserPromptSubmit"),
        "Codex config must contain UserPromptSubmit"
    );
    assert!(body.contains("5000"), "Codex config timeout must be 5000ms");

    // Verify universal prepare-commit-msg hook is installed
    let resp = client
        .get_blob("bob", "codexrepo", ".githooks/prepare-commit-msg")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Oxigit universal prepare-commit-msg hook"),
        "Codex should install the universal prepare-commit-msg hook"
    );
}

// ---------------------------------------------------------------------------
// Gemini hooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_gemini_hooks() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "carol", "gemrepo").await;

    let resp = client
        .install_ai_hook("carol", "gemrepo", "gemini-cli")
        .await;
    let status = resp.status();
    assert!(
        status.is_success() || status.is_redirection(),
        "install_ai_hook failed: {status}"
    );

    // Verify session capture script outputs JSON (Gemini CLI requirement)
    let resp = client
        .get_blob("carol", "gemrepo", ".gemini/hooks/oxigit-session.sh")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("decision") && body.contains("allow"),
        "Gemini hook must output JSON decision:allow"
    );

    // Verify config with BeforeAgent event and ms timeout
    let resp = client
        .get_blob("carol", "gemrepo", ".gemini/settings.json")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("BeforeAgent"),
        "Gemini config must contain BeforeAgent"
    );
    assert!(
        body.contains("5000"),
        "Gemini config timeout must be 5000ms"
    );

    // Verify universal prepare-commit-msg hook
    let resp = client
        .get_blob("carol", "gemrepo", ".githooks/prepare-commit-msg")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Oxigit universal prepare-commit-msg hook"),
        "Gemini should install the universal prepare-commit-msg hook"
    );
}

// ---------------------------------------------------------------------------
// All three providers coexist
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_all_three_providers_coexist() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "dave", "allhooks").await;

    // Install all three
    for tool in &["claude-code", "codex", "gemini-cli"] {
        let resp = client.install_ai_hook("dave", "allhooks", tool).await;
        let status = resp.status();
        assert!(
            status.is_success() || status.is_redirection(),
            "install_ai_hook({tool}) failed: {status}"
        );
    }

    // Verify each provider's files exist
    let resp = client
        .get_blob("dave", "allhooks", ".claude/hooks/oxigit-context.sh")
        .await;
    assert!(
        resp.text().await.unwrap().contains("oxigit-pending.json"),
        "Claude hook must exist"
    );

    let resp = client
        .get_blob("dave", "allhooks", ".codex/hooks/oxigit-session.sh")
        .await;
    assert!(
        resp.text().await.unwrap().contains("last-ai-session.json"),
        "Codex hook must exist"
    );

    let resp = client
        .get_blob("dave", "allhooks", ".gemini/hooks/oxigit-session.sh")
        .await;
    assert!(
        resp.text().await.unwrap().contains("decision"),
        "Gemini hook must exist"
    );

    // check_installed_hooks should return 3 entries, all up_to_date
    let resp = client.check_installed_hooks("dave", "allhooks").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "check_installed_hooks must list claude-code"
    );
    assert!(
        body.contains("codex"),
        "check_installed_hooks must list codex"
    );
    assert!(
        body.contains("gemini-cli"),
        "check_installed_hooks must list gemini-cli"
    );
    // All should be up_to_date (true appears for each)
    let true_count = body.matches("true").count();
    assert!(
        true_count >= 3,
        "All 3 hooks should be up_to_date, got body: {}",
        &body[..500.min(body.len())]
    );
}

// ---------------------------------------------------------------------------
// Hook status lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_check_hooks_status_lifecycle() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "eve", "lifecycle").await;

    // No hooks installed yet — should return empty list
    let resp = client.check_installed_hooks("eve", "lifecycle").await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("claude-code") && !body.contains("codex") && !body.contains("gemini-cli"),
        "No hooks should be reported initially"
    );

    // Install claude-code
    client
        .install_ai_hook("eve", "lifecycle", "claude-code")
        .await;
    let resp = client.check_installed_hooks("eve", "lifecycle").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code"),
        "claude-code should appear after install"
    );
    assert!(!body.contains("codex"), "codex should not appear yet");

    // Install codex
    client.install_ai_hook("eve", "lifecycle", "codex").await;
    let resp = client.check_installed_hooks("eve", "lifecycle").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code") && body.contains("codex"),
        "Both claude-code and codex should appear"
    );
}

// ---------------------------------------------------------------------------
// Reinstall stays up to date
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_reinstall_hooks_stays_up_to_date() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "frank", "reinstall").await;

    // Install then reinstall
    client
        .install_ai_hook("frank", "reinstall", "claude-code")
        .await;
    client
        .install_ai_hook("frank", "reinstall", "claude-code")
        .await;

    let resp = client.check_installed_hooks("frank", "reinstall").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("claude-code") && body.contains("true"),
        "Reinstalled hook should still be up_to_date"
    );
}

// ---------------------------------------------------------------------------
// Access control: non-owner cannot install hooks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_hooks_requires_owner() {
    let server = TestServer::start().await;
    let (client_alice, _dest) = setup_user_with_repo(&server, "gail", "private").await;
    drop(client_alice);

    // Register and login as a different user
    let client_bob = server.client();
    client_bob
        .register("hank", "hank@test.com", "password123")
        .await;
    client_bob.login("hank", "password123").await;

    // Bob tries to install hooks on Gail's repo
    let resp = client_bob
        .install_ai_hook("gail", "private", "claude-code")
        .await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    // Should fail — either error status or error message in body
    let is_error = status.is_client_error()
        || status.is_server_error()
        || body.to_lowercase().contains("error")
        || body.to_lowercase().contains("denied")
        || body.to_lowercase().contains("owner");
    assert!(
        is_error,
        "Non-owner should not be able to install hooks, got status={status}, body={}",
        &body[..300.min(body.len())]
    );
}

// ---------------------------------------------------------------------------
// Edge: install on empty repo (no branches)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_hooks_on_empty_repo_fails() {
    let server = TestServer::start().await;
    let client = server.client();
    client
        .register("iris", "iris@test.com", "password123")
        .await;
    client.login("iris", "password123").await;
    client.create_repo("emptyrepo", "no commits", false).await;

    // No initial push — repo has no branches
    let resp = client
        .install_ai_hook("iris", "emptyrepo", "claude-code")
        .await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    let is_error = status.is_client_error()
        || status.is_server_error()
        || body.to_lowercase().contains("error")
        || body.to_lowercase().contains("branch");
    assert!(
        is_error,
        "Installing hooks on empty repo should fail, got status={status}, body={}",
        &body[..300.min(body.len())]
    );
}

// ---------------------------------------------------------------------------
// Edge: unknown tool_id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_install_unknown_tool_fails() {
    let server = TestServer::start().await;
    let (client, _dest) = setup_user_with_repo(&server, "jack", "tooltest").await;

    let resp = client
        .install_ai_hook("jack", "tooltest", "nonexistent-tool")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.to_lowercase().contains("error") || body.to_lowercase().contains("unknown"),
        "Unknown tool should fail, got: {}",
        &body[..300.min(body.len())]
    );
}
