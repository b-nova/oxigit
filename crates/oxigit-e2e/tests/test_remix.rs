mod harness;

use harness::*;

/// Test: forking a repo with REMIX.md redirects to the remix guide page.
#[tokio::test]
async fn remix_fork_redirects_to_guide() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    // Alice creates a repo with REMIX.md
    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    alice.create_repo("starter", "starter project", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "starter");
    let repo_dir = tmp.path().join("starter");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "README.md", "# Starter\n", "add readme");
    create_commit(
        &repo_dir,
        "REMIX.md",
        "# Getting Started\n\nClone and run the project.\n",
        "add remix guide",
    );
    git_push(&repo_dir);

    // Bob forks — should redirect to remix-guide
    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    let resp = bob.fork_repo("alice", "starter").await;

    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.contains("/bob/starter/remix-guide"),
        "fork of repo with REMIX.md should redirect to remix-guide, got location={location}"
    );
}

/// Test: remix guide page shows content, clone URL, and fork source.
#[tokio::test]
async fn remix_guide_shows_content_and_clone_url() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    alice.create_repo("starter", "starter project", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "starter");
    let repo_dir = tmp.path().join("starter");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "README.md", "# Starter\n", "add readme");
    create_commit(
        &repo_dir,
        "REMIX.md",
        "# Getting Started\n\nClone and run the project.\n",
        "add remix guide",
    );
    git_push(&repo_dir);

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    bob.fork_repo("alice", "starter").await;

    // Fetch the remix guide data via server function
    let resp = bob.fetch_remix_guide("bob", "starter").await;
    let body = resp.text().await.unwrap();

    // Server function returns JSON with remix_html, clone_url, source_name
    assert!(
        body.contains("remix_html") || body.contains("Getting Started"),
        "remix guide should contain rendered markdown: {body}"
    );
    assert!(
        body.contains("bob/starter.git"),
        "remix guide should contain clone URL: {body}"
    );
    assert!(
        body.contains("alice/starter"),
        "remix guide should show fork source: {body}"
    );
}

/// Test: forking a repo without REMIX.md does NOT redirect to remix guide.
#[tokio::test]
async fn fork_without_remix_md_skips_guide() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    alice.create_repo("plain", "no remix", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "plain");
    let repo_dir = tmp.path().join("plain");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "README.md", "# Plain\n", "add readme");
    git_push(&repo_dir);

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    let resp = bob.fork_repo("alice", "plain").await;

    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        !location.contains("remix-guide"),
        "fork without REMIX.md should not redirect to remix-guide, got location={location}"
    );
}

/// Test: remix guide renders REMIX.md markdown as HTML.
#[tokio::test]
async fn remix_guide_renders_markdown_html() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    let alice = server.client();
    alice.register("alice", "alice@test.com", "password123").await;
    alice.create_repo("template", "template project", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "template");
    let repo_dir = tmp.path().join("template");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "README.md", "# Template\n", "add readme");
    create_commit(
        &repo_dir,
        "REMIX.md",
        "# Getting Started\n\n- Step 1\n- Step 2\n- Step 3\n",
        "add remix guide",
    );
    git_push(&repo_dir);

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;
    bob.fork_repo("alice", "template").await;

    let resp = bob.fetch_remix_guide("bob", "template").await;
    let body = resp.text().await.unwrap();
    let body = strip_hydration_markers(&body);

    // Markdown should be rendered as HTML
    assert!(
        body.contains("<h1>") || body.contains("Getting Started"),
        "REMIX.md heading should be rendered as HTML: {body}"
    );
    assert!(
        body.contains("<li>") || body.contains("Step 1"),
        "REMIX.md list items should be rendered as HTML: {body}"
    );
}
