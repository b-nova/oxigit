mod harness;

use harness::TestServer;

#[tokio::test]
async fn test_create_public_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let resp = client
        .create_repo("myproject", "A test project", false)
        .await;
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "create public repo failed with status: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_create_private_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let resp = client
        .create_repo("secret-project", "Private repo", true)
        .await;
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "create private repo failed with status: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_create_repo_unauthenticated() {
    let server = TestServer::start().await;
    let client = server.client();

    // No login — should fail
    let resp = client.create_repo("nope", "Should fail", false).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("authenticated")
            || body.contains("login"),
        "expected auth error, got: {body}"
    );
}

#[tokio::test]
async fn test_create_duplicate_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("myproject", "First", false).await;

    let resp = client.create_repo("myproject", "Second", false).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("exists")
            || body.contains("already"),
        "expected duplicate repo error, got: {body}"
    );
}
