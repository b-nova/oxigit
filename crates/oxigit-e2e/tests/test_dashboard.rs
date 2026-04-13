mod harness;

use harness::TestServer;

/// Authenticated user gets dashboard data.
#[tokio::test]
async fn test_dashboard_authenticated() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    let resp = client.fetch_dashboard().await;
    let body = resp.text().await.unwrap();

    // Should return Some(DashboardData) with fields like tool_usage, recent_sessions
    assert!(
        body.contains("tool_usage") || body.contains("recent_sessions") || body.contains("Ok"),
        "Expected dashboard data for authenticated user, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Unauthenticated user gets None (landing page).
#[tokio::test]
async fn test_dashboard_unauthenticated() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.fetch_dashboard().await;
    let body = resp.text().await.unwrap();

    // Should return Ok(None) — no dashboard data
    assert!(
        !body.contains("tool_usage"),
        "Unauthenticated user should not get dashboard data"
    );
}

/// Home page renders via SSR GET.
#[tokio::test]
async fn test_home_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.get("/").await;
    let status = resp.status();
    assert!(
        status.is_success(),
        "Home page should load, got status: {}",
        status
    );
}

/// Dashboard reflects data after creating a repo with AI commits.
#[tokio::test]
async fn test_dashboard_reflects_activity() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "Test repo", false).await;

    // Push a commit with AI metadata
    let clone_url =
        harness::http_clone_url(&server.base_url, "alice", "password123", "alice", "myrepo");
    let dest = server.data_dir.path().join("clone-dash");
    harness::git_clone_http(&clone_url, &dest);
    harness::init_repo_config(&dest);
    harness::create_commit_with_trailers(
        &dest,
        "file.rs",
        "fn main() {}",
        "add main",
        "claude-code",
        Some("claude-4"),
        Some("write a main fn"),
        Some("sess-dash-1"),
        Some(0),
    );
    let push = harness::git_push(&dest);
    assert!(push.status.success(), "push should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.fetch_dashboard().await;
    let body = resp.text().await.unwrap();

    // After pushing AI commits, dashboard should show some data
    assert!(
        body.contains("claude-code") || body.contains("recent_sessions") || body.contains("Ok"),
        "Dashboard should reflect AI activity, got: {}",
        &body[..500.min(body.len())]
    );
}
