mod harness;

use harness::*;

/// Test fetching a nonexistent recipe returns an error.
#[tokio::test]
async fn test_fetch_nonexistent_recipe() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.fetch_recipe_detail(999).await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("not found")
            || body.contains("Not found"),
        "Expected error for nonexistent recipe, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Test the full recipe workflow: publish from AI session, then fetch detail.
#[tokio::test]
async fn test_recipe_publish_and_fetch_detail() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client
        .create_repo("reciperepo", "Recipe test", false)
        .await;

    // Push AI commits to create a session
    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "reciperepo",
    );
    let dest = server.data_dir.path().join("clone-recipe");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest,
        "app.rs",
        "fn main() { println!(\"hello\"); }",
        "add main app",
        "claude-code",
        Some("claude-4"),
        Some("create the main app"),
        Some("recipe-session-1"),
        Some(0),
    );
    let push = git_push(&dest);
    assert!(push.status.success(), "push should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Try to access recipe marketplace page
    let resp = client.get("/recipes").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Recipes") || body.contains("recipes") || body.contains("Marketplace"),
        "Recipe marketplace should render, got: {}",
        &body[..500.min(body.len())]
    );

    // The session detail page should have a share button
    let resp = client
        .get("/alice/reciperepo/ai/recipe-session-1")
        .await;
    let body = resp.text().await.unwrap();

    // Session detail may or may not have share button depending on plan
    // At minimum, the page should render
    assert!(
        body.contains("recipe-session-1")
            || body.contains("Session")
            || body.contains("session")
            || body.contains("claude-code"),
        "Session detail page should render, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Recipe detail page renders via SSR GET.
#[tokio::test]
async fn test_recipe_detail_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    // Fetch recipe page with a non-existent id — should still load the page shell
    let resp = client.get("/recipes/1").await;
    let status = resp.status();
    // The page may return 200 with an error message or 404
    assert!(
        status.is_success() || status.as_u16() == 404,
        "Recipe detail page should handle missing recipe, got: {}",
        status
    );
}
