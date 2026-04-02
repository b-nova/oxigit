mod harness;

use harness::*;

/// Test that the blame page shows AI attribution for AI-generated lines.
#[tokio::test]
async fn test_blame_shows_ai_attribution() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("blamerepo", "Blame test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "blamerepo");
    let dest = server.data_dir.path().join("clone-blame");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    // Create a human commit first
    create_commit(&dest, "app.rs", "fn main() {}\n", "initial commit");

    // Then an AI commit that modifies the file
    create_commit_with_trailers(
        &dest,
        "app.rs",
        "fn main() {\n    println!(\"hello\");\n}\n",
        "feat: add greeting",
        "claude-code",
        Some("claude-opus-4-6"),
        Some("Add a greeting to main"),
        Some("blame-session-1"),
        None,
    );

    let push = git_push(&dest);
    assert!(push.status.success(), "push failed");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch blame page
    let resp = client.get("/alice/blamerepo/blame/app.rs").await;
    let body = resp.text().await.unwrap();

    // Should show the file content
    assert!(body.contains("app.rs"), "Expected file name in blame view");
    // Should show AI attribution
    assert!(body.contains("claude-code"), "Expected AI tool badge in blame, got: {}", &body[..500.min(body.len())]);
    // Should show percentage stats
    assert!(body.contains("AI-generated") || body.contains("AI lines"), "Expected AI stats in blame view");
}

/// Test that blame page works for files with no AI-generated lines.
#[tokio::test]
async fn test_blame_no_ai_lines() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.login("bob", "password123").await;
    client.create_repo("humanrepo", "Human-only repo", false).await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "humanrepo");
    let dest = server.data_dir.path().join("clone-human");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit(&dest, "manual.rs", "fn hand_written() {}\n", "human commit");
    let push = git_push(&dest);
    assert!(push.status.success());

    let resp = client.get("/bob/humanrepo/blame/manual.rs").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("manual.rs"), "Expected file name");
    assert!(body.contains("No AI-generated lines"), "Expected no-AI message for human-only file");
}

/// Test that blob view includes a Blame button.
#[tokio::test]
async fn test_blob_view_has_blame_button() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("carol", "carol@test.com", "password123").await;
    client.login("carol", "password123").await;
    client.create_repo("btnrepo", "Button test", false).await;

    let clone_url = http_clone_url(&server.base_url, "carol", "password123", "carol", "btnrepo");
    let dest = server.data_dir.path().join("clone-btn");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit(&dest, "code.rs", "fn test() {}", "add code");
    let push = git_push(&dest);
    assert!(push.status.success());

    let resp = client.get("/carol/btnrepo/blob/code.rs").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("Blame"), "Expected Blame button in blob view");
    assert!(body.contains("/carol/btnrepo/blame/code.rs"), "Expected Blame link URL");
}
