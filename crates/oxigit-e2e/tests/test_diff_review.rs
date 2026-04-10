mod harness;

use harness::*;

/// Test: diff review flags a hardcoded secret in an added line.
#[tokio::test]
async fn diff_review_flags_hardcoded_secret() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "myrepo");
    let repo_dir = tmp.path().join("myrepo");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    create_commit(
        &repo_dir,
        "config.py",
        "api_key = \"supersecret123456789\"\n",
        "add config with secret",
    );
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    let resp = client.get_diff_review("alice", "myrepo", &sha).await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("security"),
        "diff review should flag a security risk for hardcoded secret: {body}"
    );
    assert!(
        body.contains("hardcoded secret") || body.contains("API key"),
        "diff review should mention the hardcoded secret pattern: {body}"
    );
}

/// Test: diff review returns no risk flags for a clean commit.
#[tokio::test]
async fn diff_review_clean_commit_no_flags() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("clean", "test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "clean");
    let repo_dir = tmp.path().join("clean");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    create_commit(&repo_dir, "hello.txt", "Hello, world!\n", "add greeting");
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    let resp = client.get_diff_review("alice", "clean", &sha).await;
    let body = resp.text().await.unwrap();

    assert!(
        !body.contains("security"),
        "clean commit should have no security flags: {body}"
    );
    assert!(
        !body.contains("breaking"),
        "clean commit should have no breaking flags: {body}"
    );
    assert!(
        !body.contains("quality"),
        "clean commit should have no quality flags: {body}"
    );
}

/// Test: diff review flags removal of a public function as a breaking change.
#[tokio::test]
async fn diff_review_flags_removed_public_api() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("api", "test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "api");
    let repo_dir = tmp.path().join("api");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    // First commit: add a public function
    create_commit(
        &repo_dir,
        "lib.rs",
        "pub fn helper() {\n    println!(\"helper\");\n}\n\npub fn main_fn() {\n    helper();\n}\n",
        "add public API",
    );
    git_push(&repo_dir);

    // Second commit: remove the public function
    create_commit(
        &repo_dir,
        "lib.rs",
        "pub fn main_fn() {\n    println!(\"inline\");\n}\n",
        "remove helper",
    );
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    let resp = client.get_diff_review("alice", "api", &sha).await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("breaking"),
        "removing a pub fn should flag as breaking change: {body}"
    );
}

/// Test: diff review flags TODO comments as quality issues.
#[tokio::test]
async fn diff_review_flags_todo_comment() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("todos", "test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "todos");
    let repo_dir = tmp.path().join("todos");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    create_commit(
        &repo_dir,
        "main.rs",
        "fn main() {\n    // TODO: fix this later\n    println!(\"wip\");\n}\n",
        "add wip code",
    );
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    let resp = client.get_diff_review("alice", "todos", &sha).await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("quality"),
        "TODO comment should flag as quality issue: {body}"
    );
}

/// Test: generating a diff summary requires a Flat or higher plan (SaaS only).
#[tokio::test]
async fn generate_summary_requires_flat_plan() {
    let server = TestServer::start().await;
    if !server.has_saas().await {
        eprintln!("SKIPPED: server built without saas feature");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;
    client.create_repo("gated", "test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "gated");
    let repo_dir = tmp.path().join("gated");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);

    create_commit(&repo_dir, "file.txt", "content\n", "init");
    git_push(&repo_dir);
    let sha = get_head_sha(&repo_dir);

    // Free user should be blocked from generating summaries
    let resp = client.generate_diff_summary("alice", "gated", &sha).await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Flat") || body.contains("higher plan") || body.contains("Upgrade"),
        "free user should see plan upgrade message for diff summary: {body}"
    );
}
