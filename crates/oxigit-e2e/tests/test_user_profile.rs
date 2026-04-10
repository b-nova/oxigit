mod harness;

use harness::*;

/// Test: user profile shows public repos.
#[tokio::test]
async fn user_profile_shows_public_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client
        .create_repo("public-repo", "a public repo", false)
        .await;
    client.create_repo("another", "another public", false).await;

    // Fetch profile (even as anonymous)
    let anon = server.client();
    let resp = anon.fetch_user_profile("alice").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("public-repo"),
        "should show public repo: {body}"
    );
    assert!(
        body.contains("another"),
        "should show another public repo: {body}"
    );
}

/// Test: user profile hides private repos from others.
#[tokio::test]
async fn user_profile_hides_private_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.create_repo("visible", "public", false).await;
    client.create_repo("hidden", "private", true).await;

    // Anonymous user
    let anon = server.client();
    let resp = anon.fetch_user_profile("bob").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("visible"), "should show public repo");
    assert!(
        !body.contains("hidden"),
        "should NOT show private repo to anonymous: {body}"
    );
}

/// Test: user profile shows own private repos.
#[tokio::test]
async fn user_profile_shows_own_private_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("carol", "carol@test.com", "password123")
        .await;
    client.create_repo("mypriv", "my private", true).await;

    // Same user's session
    let resp = client.fetch_user_profile("carol").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("mypriv"),
        "should show own private repo: {body}"
    );
}

/// Test: nonexistent user returns error.
#[tokio::test]
async fn user_profile_nonexistent_user() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.fetch_user_profile("nobody").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("not found") || body.contains("Not found") || body.contains("error"),
        "should error for nonexistent user: {body}"
    );
}
