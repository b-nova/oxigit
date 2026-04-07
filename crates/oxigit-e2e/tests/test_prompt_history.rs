mod harness;

use harness::*;

/// Test that the prompt history page shows prompts from pushed AI commits.
#[tokio::test]
async fn test_prompt_history_shows_prompts() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("promptrepo", "Prompt history test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "promptrepo");
    let dest = server.data_dir.path().join("clone-prompt");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Create commits with different prompts in the same session
    create_commit_with_trailers(
        &dest,
        "auth.rs",
        "fn login() {}",
        "feat: add login",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add user authentication"),
        Some("session-prompt-1"),
        Some(1),
    );
    create_commit_with_trailers(
        &dest,
        "auth.rs",
        "fn login() {}\nfn logout() {}",
        "feat: add logout",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Now add a logout function"),
        Some("session-prompt-1"),
        Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success(), "push failed");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch prompt history page
    let resp = client.get("/alice/promptrepo/prompts").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("Add user authentication"), "Expected first prompt in history, got: {}", &body[..300.min(body.len())]);
    assert!(body.contains("Now add a logout function"), "Expected second prompt in history");
    assert!(body.contains("claude-code"), "Expected AI tool badge");
}

/// Test that prompt history search filters by prompt text.
#[tokio::test]
async fn test_prompt_history_empty_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.login("bob", "password123").await;
    client.create_repo("emptyrepo", "Empty repo", false).await;

    // Push a non-AI commit
    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "emptyrepo");
    let dest = server.data_dir.path().join("clone-empty");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);
    create_commit(&dest, "readme.txt", "hello", "initial commit");
    git_push(&dest);

    let resp = client.get("/bob/emptyrepo/prompts").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("No AI prompts yet"), "Expected empty state message");
}

/// Test that multiple prompts within a session are grouped correctly.
#[tokio::test]
async fn test_prompt_history_groups_by_prompt_index() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("carol", "carol@test.com", "password123").await;
    client.login("carol", "password123").await;
    client.create_repo("grouprepo", "Grouping test", false).await;

    let clone_url = http_clone_url(&server.base_url, "carol", "password123", "carol", "grouprepo");
    let dest = server.data_dir.path().join("clone-group");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Two commits from the same prompt (prompt_index=1)
    create_commit_with_trailers(
        &dest, "a.rs", "fn a() {}", "step 1",
        "cursor", None, Some("Build feature A"), Some("sess-1"), Some(1),
    );
    create_commit_with_trailers(
        &dest, "a_test.rs", "fn test_a() {}", "step 1 tests",
        "cursor", None, Some("Build feature A"), Some("sess-1"), Some(1),
    );
    // One commit from a different prompt (prompt_index=2)
    create_commit_with_trailers(
        &dest, "b.rs", "fn b() {}", "step 2",
        "cursor", None, Some("Now build feature B"), Some("sess-1"), Some(2),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/carol/grouprepo/prompts").await;
    let body = strip_hydration_markers(&resp.text().await.unwrap());
    assert!(body.contains("Build feature A"), "Expected prompt A");
    assert!(body.contains("Now build feature B"), "Expected prompt B");
    // The "Build feature A" prompt should show 2 commits
    assert!(body.contains("2 commits"), "Expected 2 commits for prompt A");
}
