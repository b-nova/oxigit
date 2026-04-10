mod harness;

use harness::TestServer;

#[tokio::test]
async fn test_register_user() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client
        .register("alice", "alice@example.com", "password123")
        .await;
    // Leptos server functions that redirect return 3xx
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "register failed with status: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;

    // Second registration with same username should fail
    let resp = client
        .register("alice", "alice2@example.com", "password123")
        .await;
    let body = resp.text().await.unwrap();
    // The response should contain an error (not a redirect to /repos)
    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("taken")
            || body.contains("already"),
        "expected error for duplicate username, got: {body}"
    );
}

#[tokio::test]
async fn test_register_short_password() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.register("alice", "alice@example.com", "short").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error") || body.contains("Error") || body.contains("8"),
        "expected password validation error, got: {body}"
    );
}

#[tokio::test]
async fn test_login_valid() {
    let server = TestServer::start().await;
    let client = server.client();

    // Register first
    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;

    // Login with a fresh client to avoid existing session
    let client2 = server.client();
    let resp = client2.login("alice", "password123").await;
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "login failed with status: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_login_invalid_password() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;

    let client2 = server.client();
    let resp = client2.login("alice", "wrongpassword").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("Invalid")
            || body.contains("invalid"),
        "expected auth error, got: {body}"
    );
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.login("nonexistent", "password123").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("Invalid")
            || body.contains("invalid"),
        "expected error for nonexistent user, got: {body}"
    );
}
