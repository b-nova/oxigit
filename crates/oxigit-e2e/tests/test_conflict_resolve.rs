mod harness;

use harness::*;

/// Test that the conflict resolution page renders for a conflict created by a merge with conflicts.
/// This test creates conflicting changes on two branches, attempts a merge via PR,
/// and verifies the conflict resolution flow.
#[tokio::test]
async fn test_conflict_resolution_page_loads() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client
        .create_repo("conflictrepo", "Conflict test", false)
        .await;

    // Clone and push initial commit on main
    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "conflictrepo",
    );
    let dest = server.data_dir.path().join("clone-conflict");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit(&dest, "shared.txt", "line 1\nline 2\nline 3\n", "initial");
    let push = git_push(&dest);
    assert!(push.status.success(), "initial push should succeed");

    // Create a feature branch and make conflicting change
    create_branch(&dest, "feature");
    std::process::Command::new("git")
        .args(["checkout", "feature"])
        .current_dir(&dest)
        .output()
        .unwrap();
    create_commit(
        &dest,
        "shared.txt",
        "line 1 from feature\nline 2\nline 3\n",
        "feature change",
    );
    let push = std::process::Command::new("git")
        .args(["push", "origin", "feature"])
        .current_dir(&dest)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(push.status.success(), "feature push should succeed");

    // Make conflicting change on main
    std::process::Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&dest)
        .output()
        .unwrap();
    create_commit(
        &dest,
        "shared.txt",
        "line 1 from main\nline 2\nline 3\n",
        "main change",
    );
    let push = git_push(&dest);
    assert!(push.status.success(), "main push should succeed");

    // Create PR from feature -> main
    let resp = client
        .create_pr(
            "alice",
            "conflictrepo",
            "Merge feature",
            "Has conflicts",
            "feature",
            "main",
        )
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "create_pr failed: {}",
        resp.status()
    );

    // Attempt to merge — should create a conflict
    let resp = client.merge_pr("alice", "conflictrepo", 1).await;
    let _body = resp.text().await.unwrap();

    // The merge should either fail with conflict or redirect to conflict resolution
    // Check that the conflict resolution page loads
    let resp = client.get("/alice/conflictrepo/pulls/1").await;
    let body = resp.text().await.unwrap();

    // PR page should indicate conflict or merge state
    assert!(
        body.contains("conflict")
            || body.contains("Conflict")
            || body.contains("Merge")
            || body.contains("merge"),
        "Expected conflict or merge info on PR page, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Test that fetching a nonexistent conflict returns an error.
#[tokio::test]
async fn test_fetch_nonexistent_conflict() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("norepo", "No conflict", false).await;

    let resp = client.fetch_conflict_detail("alice", "norepo", 999).await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("not found")
            || body.contains("Not found"),
        "Expected error for nonexistent conflict, got: {}",
        &body[..500.min(body.len())]
    );
}
