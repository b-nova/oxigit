mod harness;

use harness::*;

/// Test: create an issue and see it in the list.
#[tokio::test]
async fn create_and_list_issue() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("myproject", "test", false).await;

    let resp = client.create_issue("alice", "myproject", "Bug report", "Something is broken").await;
    assert!(resp.status().is_success() || resp.status().is_redirection());

    let resp = client.list_issues("alice", "myproject", "open").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("Bug report"), "issue should appear in list: {body}");
}

/// Test: view issue detail.
#[tokio::test]
async fn view_issue_detail() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("myproject", "test", false).await;
    client.create_issue("alice", "myproject", "Feature request", "Please add dark mode").await;

    let resp = client.get_issue("alice", "myproject", 1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("Feature request"), "should show title: {body}");
    assert!(body.contains("dark mode"), "should show description: {body}");
}

/// Test: add comment to issue.
#[tokio::test]
async fn add_comment_to_issue() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("myproject", "test", false).await;
    client.create_issue("alice", "myproject", "Discussion", "Let's discuss").await;

    let resp = client.add_issue_comment("alice", "myproject", 1, "Great idea!").await;
    assert!(resp.status().is_success() || resp.status().is_redirection());

    let resp = client.get_issue("alice", "myproject", 1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("Great idea!"), "comment should appear: {body}");
}

/// Test: close and reopen issue.
#[tokio::test]
async fn close_and_reopen_issue() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("myproject", "test", false).await;
    client.create_issue("alice", "myproject", "To close", "Will be closed").await;

    // Close
    let resp = client.close_issue("alice", "myproject", 1).await;
    assert!(resp.status().is_success() || resp.status().is_redirection());

    let resp = client.get_issue("alice", "myproject", 1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("closed"), "issue should be closed: {body}");

    // Reopen
    let resp = client.reopen_issue("alice", "myproject", 1).await;
    assert!(resp.status().is_success() || resp.status().is_redirection());

    let resp = client.get_issue("alice", "myproject", 1).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("open"), "issue should be open: {body}");
}

/// Test: issue numbers auto-increment.
#[tokio::test]
async fn issue_numbers_increment() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("myproject", "test", false).await;

    client.create_issue("alice", "myproject", "Issue one", "").await;
    client.create_issue("alice", "myproject", "Issue two", "").await;
    client.create_issue("alice", "myproject", "Issue three", "").await;

    let resp = client.get_issue("alice", "myproject", 3).await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("Issue three"), "issue #3 should exist: {body}");
}

/// Test: private repo issues not accessible without auth.
#[tokio::test]
async fn private_repo_issues_denied() {
    let server = TestServer::start().await;
    let alice = server.client();

    alice.register("alice", "alice@test.com", "password123").await;
    alice.create_repo("secret", "private", true).await;
    alice.create_issue("alice", "secret", "Secret issue", "").await;

    let anon = server.client();
    let resp = anon.list_issues("alice", "secret", "open").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("not found") || body.contains("Not found") || body.contains("error") || body.is_empty(),
        "anonymous should not see private repo issues: {body}"
    );
}
