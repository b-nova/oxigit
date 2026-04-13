mod harness;

use harness::TestServer;

/// User can fetch their profile.
#[tokio::test]
async fn test_fetch_profile() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    let resp = client.fetch_profile().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("alice"),
        "Expected profile to contain username, got: {}",
        &body[..500.min(body.len())]
    );
}

/// User can update display name and email.
#[tokio::test]
async fn test_save_profile() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    let resp = client
        .save_profile("Alice Wonderland", "newalice@test.com")
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "save_profile failed: {}",
        resp.status()
    );

    // Verify profile was updated
    let resp = client.fetch_profile().await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Alice Wonderland") || body.contains("newalice"),
        "Profile should reflect updated data, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Save profile with invalid email fails.
#[tokio::test]
async fn test_save_profile_invalid_email() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    let resp = client.save_profile("Alice", "not-an-email").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error") || body.contains("Error") || body.contains("valid email"),
        "Expected validation error for invalid email, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Unauthenticated user cannot fetch profile.
#[tokio::test]
async fn test_fetch_profile_unauthenticated() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.fetch_profile().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("Not authenticated")
            || body.contains("login"),
        "Unauthenticated user should be denied, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Profile edit page renders via SSR GET.
#[tokio::test]
async fn test_profile_edit_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    let resp = client.get("/profile").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Profile"),
        "Expected profile page, got: {}",
        &body[..500.min(body.len())]
    );
}
