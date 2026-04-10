mod harness;

use harness::*;
use std::process::Command;

/// Helper: push a branch to a repo.
fn git_push_branch(repo_dir: &std::path::Path, branch: &str) -> std::process::Output {
    Command::new("git")
        .args(["push", "origin", branch])
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git push branch failed")
}

/// Helper: create and checkout a new branch.
fn git_checkout_new_branch(repo_dir: &std::path::Path, branch: &str) {
    let output = Command::new("git")
        .args(["checkout", "-b", branch])
        .current_dir(repo_dir)
        .output()
        .expect("git checkout -b failed");
    assert!(
        output.status.success(),
        "git checkout -b failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test: create a PR, list it, view it.
#[tokio::test]
async fn create_and_view_pr() {
    let server = TestServer::start().await;
    let client = server.client();
    let tmp = tempfile::tempdir().unwrap();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("prtest", "PR test repo", false).await;

    // Push main branch
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "prtest");
    let repo_dir = tmp.path().join("prtest");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "README.md", "# Test\n", "Initial commit");
    git_push(&repo_dir);

    // Create feature branch and push
    git_checkout_new_branch(&repo_dir, "feature");
    create_commit(&repo_dir, "feature.txt", "new feature\n", "Add feature");
    let output = git_push_branch(&repo_dir, "feature");
    assert!(
        output.status.success(),
        "push feature branch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Create PR
    let resp = client
        .create_pr(
            "alice",
            "prtest",
            "Add feature",
            "This adds a cool feature",
            "feature",
            "main",
        )
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "create PR should succeed: status={}",
        resp.status()
    );

    // List PRs
    let resp = client.list_prs("alice", "prtest", "open").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Add feature"),
        "PR should appear in list: {body}"
    );

    // View PR
    let resp = client.get_pr("alice", "prtest", 1).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Add feature"),
        "PR detail should contain title: {body}"
    );
    assert!(
        body.contains("feature"),
        "PR should reference source branch: {body}"
    );
}

/// Test: merge a PR via fast-forward.
#[tokio::test]
async fn merge_pr_fast_forward() {
    let server = TestServer::start().await;
    let client = server.client();
    let tmp = tempfile::tempdir().unwrap();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("mergeable", "merge test", false).await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "mergeable",
    );
    let repo_dir = tmp.path().join("mergeable");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "base.txt", "base\n", "Base commit");
    git_push(&repo_dir);

    git_checkout_new_branch(&repo_dir, "fix");
    create_commit(&repo_dir, "fix.txt", "bugfix\n", "Fix bug");
    let output = git_push_branch(&repo_dir, "fix");
    assert!(output.status.success());

    // Create and merge PR
    client
        .create_pr("alice", "mergeable", "Fix bug", "", "fix", "main")
        .await;

    let resp = client.merge_pr("alice", "mergeable", 1).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "merge should succeed: status={}",
        resp.status()
    );

    // Verify PR is merged
    let resp = client.get_pr("alice", "mergeable", 1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("merged"), "PR should be merged: {body}");
}

/// Test: close a PR.
#[tokio::test]
async fn close_pr() {
    let server = TestServer::start().await;
    let client = server.client();
    let tmp = tempfile::tempdir().unwrap();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("closeable", "close test", false).await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "closeable",
    );
    let repo_dir = tmp.path().join("closeable");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "base.txt", "base\n", "Base");
    git_push(&repo_dir);

    git_checkout_new_branch(&repo_dir, "wip");
    create_commit(&repo_dir, "wip.txt", "wip\n", "WIP");
    git_push_branch(&repo_dir, "wip");

    client
        .create_pr("alice", "closeable", "WIP changes", "", "wip", "main")
        .await;

    let resp = client.close_pr("alice", "closeable", 1).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "close should succeed: status={}",
        resp.status()
    );

    let resp = client.get_pr("alice", "closeable", 1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("closed"), "PR should be closed: {body}");
}

/// Test: PR validation - same branch rejected.
#[tokio::test]
async fn pr_same_branch_rejected() {
    let server = TestServer::start().await;
    let client = server.client();
    let tmp = tempfile::tempdir().unwrap();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.create_repo("samebranch", "test", false).await;

    let clone_url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "samebranch",
    );
    let repo_dir = tmp.path().join("samebranch");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "f.txt", "x\n", "init");
    git_push(&repo_dir);

    let resp = client
        .create_pr("alice", "samebranch", "Bad PR", "", "main", "main")
        .await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("differ") || body.contains("error") || body.contains("Error"),
        "same branch PR should be rejected: {body}"
    );
}
