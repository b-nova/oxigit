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

#[tokio::test]
async fn test_toggle_repo_visibility_public_to_private() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@example.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("viztoggle", "Toggle test", false).await;

    // Initially public (is_private=false)
    let resp = client.fetch_repo_visibility("alice", "viztoggle").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("false"),
        "Expected repo to be public initially, got: {}",
        &body[..200.min(body.len())]
    );

    // Toggle to private
    let resp = client
        .update_visibility("alice", "viztoggle", true)
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "update_visibility failed: {}",
        resp.status()
    );

    // Verify it's now private
    let resp = client.fetch_repo_visibility("alice", "viztoggle").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("true"),
        "Expected repo to be private after toggle, got: {}",
        &body[..200.min(body.len())]
    );
}

#[tokio::test]
async fn test_toggle_repo_visibility_private_to_public() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@example.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("privrepo", "Private", true).await;

    // Toggle to public
    let resp = client
        .update_visibility("alice", "privrepo", false)
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "update_visibility failed: {}",
        resp.status()
    );

    let resp = client.fetch_repo_visibility("alice", "privrepo").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("false"),
        "Expected repo to be public after toggle, got: {}",
        &body[..200.min(body.len())]
    );
}

#[tokio::test]
async fn test_update_visibility_non_owner_denied() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@example.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("ownedrepo", "Alice's repo", false).await;

    // Login as bob
    let client2 = server.client();
    client2
        .register("bob", "bob@example.com", "password123")
        .await;
    client2.login("bob", "password123").await;

    // Bob tries to change visibility of Alice's repo
    let resp = client2
        .update_visibility("alice", "ownedrepo", true)
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("owner")
            || body.contains("Not authorized"),
        "Non-owner should be denied, got: {}",
        &body[..500.min(body.len())]
    );
}
