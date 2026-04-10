mod harness;

use harness::*;

/// Test: user can fork another user's public repo.
#[tokio::test]
async fn fork_public_repo() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    // Alice creates a repo with content
    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("original", "the original", false).await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "original",
    );
    let repo_dir = tmp.path().join("original");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "README.md", "# Original\n", "Initial commit");
    git_push(&repo_dir);

    // Bob forks it
    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    let resp = bob.fork_repo("alice", "original").await;

    // Should redirect (successful fork)
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "fork should succeed: status={}",
        resp.status()
    );

    // Bob should now have the repo in their list
    let resp = bob.list_repos().await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("original"),
        "forked repo should appear in bob's repos: {body}"
    );

    // Bob should be able to browse the forked repo
    let resp = bob.fetch_repo_tree("bob", "original", "", "").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("README.md"),
        "forked repo should have the file: {body}"
    );
}

/// Test: cannot fork your own repo.
#[tokio::test]
async fn cannot_fork_own_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("myrepo", "mine", false).await;

    let resp = client.fork_repo("alice", "myrepo").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Cannot fork") || body.contains("error") || body.contains("own"),
        "should not be able to fork own repo: {body}"
    );
}

/// Test: cannot fork a private repo you don't own.
#[tokio::test]
async fn cannot_fork_private_repo() {
    let server = TestServer::start().await;

    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("secret", "private", true).await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    let resp = bob.fork_repo("alice", "secret").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("not found") || body.contains("Not found") || body.contains("error"),
        "should not be able to fork private repo: {body}"
    );
}

/// Test: forked repo shows "forked from" info.
#[tokio::test]
async fn fork_shows_source_info() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("upstream", "the source", false).await;

    // Push a commit so the repo isn't empty
    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "upstream",
    );
    let repo_dir = tmp.path().join("upstream");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "file.txt", "content", "init");
    git_push(&repo_dir);

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    bob.fork_repo("alice", "upstream").await;

    // Check that the forked repo's tree response includes fork source info
    let resp = bob.fetch_repo_tree("bob", "upstream", "", "").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("alice/upstream") || body.contains("forked_from"),
        "should show fork source info: {body}"
    );
}
