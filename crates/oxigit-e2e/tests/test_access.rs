mod harness;

use harness::{
    create_commit, git_clone_http, git_push, http_clone_url, init_repo_config, TestServer,
};

#[tokio::test]
async fn test_private_repo_not_visible_to_other_user() {
    let server = TestServer::start().await;

    let alice = server.client();
    let _ = alice.register("alice", "alice@example.com", "password123").await;
    let _ = alice.create_repo("secret", "Private repo", true).await;

    let bob = server.client();
    let _ = bob.register("bob", "bob@example.com", "password123").await;

    let resp = bob.fetch_repo_tree("alice", "secret", "", "").await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    // Should either be an error status, contain error text, or be empty (denied)
    assert!(
        !status.is_success() || body.contains("error") || body.contains("Error")
            || body.contains("not found") || body.contains("Not found") || body.is_empty(),
        "bob should not see alice's private repo, got status={status} body={body}"
    );
}

#[tokio::test]
async fn test_public_repo_visible_to_others() {
    let server = TestServer::start().await;

    let alice = server.client();
    let _ = alice.register("alice", "alice@example.com", "password123").await;
    let _ = alice.create_repo("public-project", "Public repo", false).await;

    let clone_dir = server.data_dir.path().join("alice_clone");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "public-project",
    );
    let output = git_clone_http(&url, &clone_dir);
    assert!(output.status.success());
    init_repo_config(&clone_dir);
    create_commit(&clone_dir, "hello.txt", "Hello!\n", "Initial commit");
    let output = git_push(&clone_dir);
    assert!(output.status.success());

    let bob = server.client();
    let _ = bob.register("bob", "bob@example.com", "password123").await;

    let resp = bob.fetch_repo_tree("alice", "public-project", "", "").await;
    assert!(
        resp.status().is_success(),
        "bob should see alice's public repo, got status: {}",
        resp.status()
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("hello.txt"),
        "response should contain the file, got: {body}"
    );
}

#[tokio::test]
async fn test_private_repo_visible_to_owner() {
    let server = TestServer::start().await;

    let alice = server.client();
    let _ = alice.register("alice", "alice@example.com", "password123").await;
    let _ = alice.create_repo("my-secret", "Private", true).await;

    let clone_dir = server.data_dir.path().join("owner_clone");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "my-secret",
    );
    let output = git_clone_http(&url, &clone_dir);
    assert!(output.status.success());
    init_repo_config(&clone_dir);
    create_commit(&clone_dir, "secret.txt", "Top secret\n", "Secret commit");
    let output = git_push(&clone_dir);
    assert!(output.status.success());

    let resp = alice.fetch_repo_tree("alice", "my-secret", "", "").await;
    assert!(
        resp.status().is_success(),
        "owner should see own private repo, got status: {}",
        resp.status()
    );
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("secret.txt"),
        "response should contain the file, got: {body}"
    );
}
