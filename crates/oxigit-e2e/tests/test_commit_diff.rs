mod harness;

use harness::*;

/// Test: viewing a commit diff shows the changes.
#[tokio::test]
async fn commit_diff_shows_changes() {
    let server = TestServer::start().await;
    let client = server.client();
    let tmp = tempfile::tempdir().unwrap();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("diffrepo", "test", false).await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "diffrepo",
    );
    let repo_dir = tmp.path().join("diffrepo");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    create_commit(&repo_dir, "hello.txt", "hello world\n", "Add hello");
    git_push(&repo_dir);

    // Get the HEAD commit SHA directly from the local repo
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo_dir)
        .output()
        .expect("git rev-parse failed");
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(sha.len(), 40, "SHA should be 40 hex chars");

    // Fetch the diff
    let resp = client.fetch_commit_diff("alice", "diffrepo", &sha).await;
    let diff_body = resp.text().await.unwrap();

    assert!(
        diff_body.contains("hello") || diff_body.contains("Add hello"),
        "diff should contain commit content or message: {diff_body}"
    );
}
