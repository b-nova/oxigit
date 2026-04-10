mod harness;

use harness::{
    TestServer, create_commit, git_clone_http, git_pull, git_push, http_clone_url,
    http_clone_url_no_auth,
};
use std::path::Path;

#[tokio::test]
async fn test_clone_empty_public_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("empty-repo", "Empty", false).await;

    let clone_dir = server.data_dir.path().join("clone_empty");
    let url = http_clone_url_no_auth(&server.base_url, "alice", "empty-repo");
    let output = git_clone_http(&url, &clone_dir);

    // git clone of an empty repo exits 0 but prints a warning to stderr
    assert!(
        output.status.success(),
        "clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn test_push_and_clone_http() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("myproject", "Test project", false).await;

    // Clone with auth
    let clone1 = server.data_dir.path().join("clone1");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "myproject",
    );
    let output = git_clone_http(&url, &clone1);
    assert!(
        output.status.success(),
        "clone1 failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Create a file, commit, push
    // First configure the repo for initial commit
    init_repo_config(&clone1);
    create_commit(&clone1, "hello.txt", "Hello, world!\n", "Initial commit");

    let output = git_push(&clone1);
    assert!(
        output.status.success(),
        "push failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Clone again to a new dir — should have the file
    let clone2 = server.data_dir.path().join("clone2");
    let url_no_auth = http_clone_url_no_auth(&server.base_url, "alice", "myproject");
    let output = git_clone_http(&url_no_auth, &clone2);
    assert!(
        output.status.success(),
        "clone2 failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        clone2.join("hello.txt").exists(),
        "hello.txt should exist in second clone"
    );
    let content = std::fs::read_to_string(clone2.join("hello.txt")).unwrap();
    assert_eq!(content, "Hello, world!\n");
}

#[tokio::test]
async fn test_push_and_pull_http() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("pulltest", "Pull test", false).await;

    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "pulltest",
    );

    // Clone twice
    let clone1 = server.data_dir.path().join("pull_clone1");
    let clone2 = server.data_dir.path().join("pull_clone2");

    let output = git_clone_http(&url, &clone1);
    assert!(output.status.success());

    init_repo_config(&clone1);
    create_commit(&clone1, "file1.txt", "first\n", "First commit");
    let output = git_push(&clone1);
    assert!(output.status.success());

    let output = git_clone_http(&url, &clone2);
    assert!(output.status.success());

    // Now push a second commit from clone1
    create_commit(&clone1, "file2.txt", "second\n", "Second commit");
    let output = git_push(&clone1);
    assert!(output.status.success());

    // Pull in clone2
    let output = git_pull(&clone2);
    assert!(
        output.status.success(),
        "pull failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        clone2.join("file2.txt").exists(),
        "file2.txt should exist after pull"
    );
}

#[tokio::test]
async fn test_push_wrong_user_http() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client
        .create_repo("alice-repo", "Alice's repo", false)
        .await;

    // Register bob
    let client2 = server.client();
    let _ = client2
        .register("bob", "bob@example.com", "password123")
        .await;

    // Clone alice's repo as bob (public, so clone works)
    let clone_dir = server.data_dir.path().join("bob_clone");
    let url = http_clone_url(
        &server.base_url,
        "bob",
        "password123",
        "alice",
        "alice-repo",
    );
    let output = git_clone_http(&url, &clone_dir);
    assert!(output.status.success());

    init_repo_config(&clone_dir);
    create_commit(&clone_dir, "hack.txt", "pwned\n", "Malicious commit");

    // Push as bob to alice's repo — should fail
    let output = git_push(&clone_dir);
    assert!(
        !output.status.success(),
        "push should have been rejected, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn test_clone_public_repo_no_auth() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("public-repo", "Public", false).await;

    // Push some content first (need auth for push)
    let clone_auth = server.data_dir.path().join("auth_clone");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "public-repo",
    );
    let output = git_clone_http(&url, &clone_auth);
    assert!(output.status.success());
    init_repo_config(&clone_auth);
    create_commit(&clone_auth, "readme.txt", "public content\n", "Add readme");
    let output = git_push(&clone_auth);
    assert!(output.status.success());

    // Now clone without auth — should work for public repo
    let clone_anon = server.data_dir.path().join("anon_clone");
    let url_no_auth = http_clone_url_no_auth(&server.base_url, "alice", "public-repo");
    let output = git_clone_http(&url_no_auth, &clone_anon);
    assert!(
        output.status.success(),
        "anonymous clone of public repo failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(clone_anon.join("readme.txt").exists());
}

#[tokio::test]
async fn test_clone_private_repo_no_auth() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("private-repo", "Private", true).await;

    // Clone without auth — should fail
    let clone_dir = server.data_dir.path().join("priv_clone");
    let url = http_clone_url_no_auth(&server.base_url, "alice", "private-repo");
    let output = git_clone_http(&url, &clone_dir);
    assert!(
        !output.status.success(),
        "anonymous clone of private repo should fail"
    );
}

/// Helper to set git config in cloned repo (needed for initial commits to empty repos).
fn init_repo_config(repo_dir: &Path) {
    use std::process::Command;
    let _ = Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_dir)
        .output();
    // Set default branch to main for empty repos
    let _ = Command::new("git")
        .args(["checkout", "-b", "main"])
        .current_dir(repo_dir)
        .output();
}
