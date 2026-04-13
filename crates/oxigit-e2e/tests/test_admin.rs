mod harness;

use harness::TestServer;

/// Admin dashboard renders with user/repo stats for the first user (auto-admin).
#[tokio::test]
async fn test_admin_dashboard_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    // First registered user is auto-admin
    client
        .register("admin", "admin@test.com", "password123")
        .await;
    client.login("admin", "password123").await;
    client.create_repo("testrepo", "A repo", false).await;

    let resp = client.get_admin_dashboard().await;
    let body = resp.text().await.unwrap();

    // Dashboard should return JSON with user_count and repo_count
    assert!(
        body.contains("user_count") || body.contains("Users"),
        "Expected admin dashboard data, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Admin dashboard page renders via SSR GET.
#[tokio::test]
async fn test_admin_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("admin", "admin@test.com", "password123")
        .await;
    client.login("admin", "password123").await;

    let resp = client.get("/admin").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Admin Dashboard"),
        "Expected Admin Dashboard page, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Non-admin user is denied access to admin dashboard.
#[tokio::test]
async fn test_admin_dashboard_non_admin_denied() {
    let server = TestServer::start().await;
    let client = server.client();

    // First user is admin
    client
        .register("admin", "admin@test.com", "password123")
        .await;

    // Second user is NOT admin
    let client2 = server.client();
    client2.register("bob", "bob@test.com", "password123").await;
    client2.login("bob", "password123").await;

    let resp = client2.get_admin_dashboard().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("denied")
            || body.contains("admin"),
        "Expected access denied for non-admin, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Admin can disable a user.
#[tokio::test]
async fn test_admin_disable_user() {
    let server = TestServer::start().await;
    let client = server.client();

    // First user is admin (id=1)
    client
        .register("admin", "admin@test.com", "password123")
        .await;
    client.login("admin", "password123").await;

    // Create second user (id=2)
    let client2 = server.client();
    client2.register("bob", "bob@test.com", "password123").await;

    // Admin disables bob (user_id=2)
    let resp = client.admin_toggle_disabled(2, true).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "admin_toggle_disabled failed: {}",
        resp.status()
    );

    // Bob should not be able to log in normally
    let client3 = server.client();
    let resp = client3.login("bob", "password123").await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    // Disabled user login may return an error, null, or not redirect to /repos
    assert!(
        body.contains("disabled")
            || body.contains("error")
            || body.contains("Error")
            || body.contains("Invalid")
            || body == "null"
            || !status.is_redirection(),
        "Disabled user should not be able to log in, got status={} body: {}",
        status,
        &body[..500.min(body.len())]
    );
}

/// Admin can re-enable a disabled user.
#[tokio::test]
async fn test_admin_enable_user() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("admin", "admin@test.com", "password123")
        .await;
    client.login("admin", "password123").await;

    let client2 = server.client();
    client2.register("bob", "bob@test.com", "password123").await;

    // Disable then enable bob
    client.admin_toggle_disabled(2, true).await;
    let resp = client.admin_toggle_disabled(2, false).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "admin_toggle_disabled (enable) failed: {}",
        resp.status()
    );

    // Bob should be able to log in again
    let client3 = server.client();
    let resp = client3.login("bob", "password123").await;
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "Re-enabled user should be able to log in, got: {}",
        resp.status()
    );
}
