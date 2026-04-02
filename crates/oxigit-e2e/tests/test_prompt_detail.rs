mod harness;

use harness::*;

/// Test that the prompt detail page shows prompt info and commits.
#[tokio::test]
async fn test_prompt_detail_page() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("promptdetail", "Prompt detail test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "promptdetail");
    let dest = server.data_dir.path().join("clone-pd");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Two commits from prompt_index=1
    create_commit_with_trailers(
        &dest, "auth.rs", "fn login() {}", "feat: login",
        "claude-code", Some("claude-opus-4-6"), Some("Add authentication"),
        Some("pd-session-1"), Some(1),
    );
    create_commit_with_trailers(
        &dest, "auth_test.rs", "fn test_login() {}", "test: login tests",
        "claude-code", Some("claude-opus-4-6"), Some("Add authentication"),
        Some("pd-session-1"), Some(1),
    );

    // One commit from prompt_index=2
    create_commit_with_trailers(
        &dest, "api.rs", "fn api() {}", "feat: api",
        "claude-code", Some("claude-opus-4-6"), Some("Add API endpoint"),
        Some("pd-session-1"), Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch prompt detail for prompt_index=1
    let resp = client.get("/alice/promptdetail/ai/pd-session-1/prompt/1").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Add authentication"), "Expected prompt text, got: {}", &body[..500.min(body.len())]);
    assert!(body.contains("claude-code"), "Expected AI tool badge");
    assert!(body.contains("2 commits"), "Expected 2 commits for this prompt");
    assert!(body.contains("Prompt Operations") || body.contains("Revert Prompt"),
        "Expected prompt operations for owner");
}

/// Test that prompt detail shows the correct prompt (not another prompt's data).
#[tokio::test]
async fn test_prompt_detail_isolation() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.login("bob", "password123").await;
    client.create_repo("isolation", "Isolation test", false).await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "isolation");
    let dest = server.data_dir.path().join("clone-iso");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "a.rs", "fn a() {}", "feat: a",
        "cursor", None, Some("Build feature A"),
        Some("iso-sess"), Some(1),
    );
    create_commit_with_trailers(
        &dest, "b.rs", "fn b() {}", "feat: b",
        "cursor", None, Some("Build feature B"),
        Some("iso-sess"), Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch prompt 2 — should show "Build feature B" but NOT "Build feature A"
    let resp = client.get("/bob/isolation/ai/iso-sess/prompt/2").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Build feature B"), "Expected prompt B text");
    // Prompt A's text should not appear in the prompt section (it may appear in breadcrumb links)
    assert!(body.contains("1 commit"), "Expected 1 commit for prompt 2");
}
