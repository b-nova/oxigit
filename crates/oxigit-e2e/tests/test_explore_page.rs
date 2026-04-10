mod harness;

use harness::*;

/// Test: explore page lists public repos.
#[tokio::test]
async fn explore_lists_public_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client
        .create_repo("public-one", "First public repo", false)
        .await;
    client
        .create_repo("public-two", "Second public repo", false)
        .await;

    // Anonymous user can explore
    let anon = server.client();
    let resp = anon.explore_repos("").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("public-one"),
        "should list first public repo: {body}"
    );
    assert!(
        body.contains("public-two"),
        "should list second public repo: {body}"
    );
}

/// Test: explore page does NOT list private repos.
#[tokio::test]
async fn explore_hides_private_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("visible", "public", false).await;
    client.create_repo("hidden", "private", true).await;

    let anon = server.client();
    let resp = anon.explore_repos("").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("visible"), "should show public repo");
    assert!(
        !body.contains("hidden"),
        "should NOT show private repo: {body}"
    );
}

/// Test: search filters by repo name.
#[tokio::test]
async fn explore_search_by_name() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client
        .create_repo("rust-project", "A Rust project", false)
        .await;
    client
        .create_repo("go-project", "A Go project", false)
        .await;

    let anon = server.client();
    let resp = anon.explore_repos("rust").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("rust-project"), "should find rust-project");
    assert!(
        !body.contains("go-project"),
        "should NOT find go-project: {body}"
    );
}

/// Test: search filters by owner username.
#[tokio::test]
async fn explore_search_by_owner() {
    let server = TestServer::start().await;

    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("myrepo", "Alice repo", false).await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    bob.create_repo("myrepo", "Bob repo", false).await;

    let anon = server.client();
    let resp = anon.explore_repos("alice").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("alice"), "should find alice's repo");
    // Both repos are named "myrepo", but only alice's should match the owner search
    // The response should contain alice's repo entry
    assert!(
        body.contains("Alice repo"),
        "should contain alice's description: {body}"
    );
}
