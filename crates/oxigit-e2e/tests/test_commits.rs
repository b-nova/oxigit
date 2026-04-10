mod harness;

use harness::*;

/// Test: commit history lists pushed commits.
#[tokio::test]
async fn commit_history_shows_commits() {
    let server = TestServer::start().await;
    let client = server.client();
    let tmp = tempfile::tempdir().unwrap();

    // Setup: register, create repo, push commits
    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("myrepo", "test repo", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "myrepo");
    let repo_dir = tmp.path().join("myrepo");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    create_commit(&repo_dir, "file1.txt", "hello", "First commit");
    create_commit(&repo_dir, "file2.txt", "world", "Second commit");
    git_push(&repo_dir);

    // Fetch commits
    let resp = client.fetch_commits("alice", "myrepo").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("First commit"),
        "should contain first commit message"
    );
    assert!(
        body.contains("Second commit"),
        "should contain second commit message"
    );
}

/// Test: commit history returns empty for repo with no commits.
#[tokio::test]
async fn commit_history_empty_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    client.create_repo("empty", "no commits", false).await;

    let resp = client.fetch_commits("bob", "empty").await;
    let body = resp.text().await.unwrap();

    // Should succeed but return empty list
    assert!(
        !body.contains("error") || body.contains("[]"),
        "should not error for empty repo"
    );
}

/// Test: cannot view commits of private repo without auth.
#[tokio::test]
async fn commit_history_private_repo_denied() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("carol", "carol@test.com", "password123")
        .await;
    client.create_repo("secret", "private repo", true).await;

    // New client without session
    let anon_client = server.client();
    let resp = anon_client.fetch_commits("carol", "secret").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("not found") || body.contains("Not found") || body.contains("error"),
        "anonymous should not see private repo commits: {body}"
    );
}
