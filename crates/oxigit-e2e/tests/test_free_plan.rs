mod harness;

use harness::*;

// ---------------------------------------------------------------------------
// Feature: Unlimited public repos
// ---------------------------------------------------------------------------

/// Test: free user can create more than 5 public repos without hitting a limit.
#[tokio::test]
async fn free_plan_unlimited_public_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    for i in 1..=6 {
        let name = format!("pub{}", i);
        let resp = client.create_repo(&name, "public repo", false).await;
        let status = resp.status().as_u16();
        assert!(
            resp.status().is_success() || resp.status().is_redirection(),
            "public repo #{i} should succeed, got {status}"
        );
        let body = resp.text().await.unwrap();
        assert!(
            !body.contains("Free plan allows"),
            "public repo #{i} should not hit plan limit: {body}"
        );
    }
}

// ---------------------------------------------------------------------------
// Feature: 5 private repos
// ---------------------------------------------------------------------------

/// Test: free user can create up to 5 private repos.
#[tokio::test]
async fn free_plan_allows_5_private_repos() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    for i in 1..=5 {
        let name = format!("priv{}", i);
        let resp = client.create_repo(&name, "private repo", true).await;
        assert!(
            resp.status().is_success() || resp.status().is_redirection(),
            "private repo #{i} should succeed, got {}",
            resp.status()
        );
        let body = resp.text().await.unwrap();
        assert!(
            !body.contains("Free plan allows"),
            "private repo #{i} should not hit plan limit: {body}"
        );
    }
}

/// Test: free user is blocked from creating a 6th private repo.
#[tokio::test]
async fn free_plan_blocks_6th_private_repo() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    // Create the allowed 5 private repos
    for i in 1..=5 {
        let name = format!("priv{}", i);
        let resp = client.create_repo(&name, "private repo", true).await;
        assert!(
            resp.status().is_success() || resp.status().is_redirection(),
            "private repo #{i} should succeed, got {}",
            resp.status()
        );
    }

    // 6th should be blocked
    let resp = client.create_repo("priv6", "one too many", true).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Free plan allows up to 5 private repositories"),
        "6th private repo should be rejected with limit message: {body}"
    );
}

// ---------------------------------------------------------------------------
// Feature: AI commit badges
// ---------------------------------------------------------------------------

/// Test: free user sees AI tool/model badges on commits.
#[tokio::test]
async fn free_plan_ai_commit_badges() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Clone, commit with AI trailers, and push
    let clone_dest = server.data_dir.path().join("clone-badges");
    let clone_url = http_clone_url(&client.base_url, "alice", "password123", "alice", "myrepo");
    git_clone_http(&clone_url, &clone_dest);
    init_repo_config(&clone_dest);

    create_commit(
        &clone_dest,
        "main.rs",
        "fn main() {}",
        "feat: initial commit\n\nOxigit-Tool: claude\nOxigit-Model: opus",
    );
    git_push(&clone_dest);

    // Fetch commits — free user should see AI tool/model badges
    let resp = client.fetch_commits("alice", "myrepo").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("claude") || body.contains("opus"),
        "Free user should see AI tool/model badges on commits: {body}"
    );
}

// ---------------------------------------------------------------------------
// Feature: AI Hub preview
// ---------------------------------------------------------------------------

/// Test: free user can access AI Hub in preview mode.
#[tokio::test]
async fn free_plan_ai_hub_preview() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Clone, push commit with AI session data so AI Hub has content
    let clone_dest = server.data_dir.path().join("clone-aihub");
    let clone_url = http_clone_url(&client.base_url, "alice", "password123", "alice", "myrepo");
    git_clone_http(&clone_url, &clone_dest);
    init_repo_config(&clone_dest);

    create_commit_with_trailers(
        &clone_dest,
        "file1.rs",
        "fn hello() {}",
        "feat: add hello",
        "claude",
        Some("opus"),
        Some("create a hello function"),
        Some("session-preview-test"),
        Some(1),
    );
    git_push(&clone_dest);

    // Access AI Hub — should work but show preview mode
    let resp = client
        .client
        .get(format!("{}/alice/myrepo/ai", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    let clean = strip_hydration_markers(&body);
    assert!(
        clean.contains("preview mode") || clean.contains("Preview mode"),
        "AI Hub should show preview mode for free users: {clean}"
    );
}

// ---------------------------------------------------------------------------
// Feature: 30-day metrics
// ---------------------------------------------------------------------------

/// Test: free user sees metrics page with 30-day limit indicator.
#[tokio::test]
async fn free_plan_30_day_metrics() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Push a commit so metrics have data
    let clone_dest = server.data_dir.path().join("clone-metrics");
    let clone_url = http_clone_url(&client.base_url, "alice", "password123", "alice", "myrepo");
    git_clone_http(&clone_url, &clone_dest);
    init_repo_config(&clone_dest);

    create_commit_with_trailers(
        &clone_dest,
        "lib.rs",
        "pub fn run() {}",
        "feat: initial",
        "claude",
        Some("opus"),
        None,
        None,
        None,
    );
    git_push(&clone_dest);

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/metrics", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    let clean = strip_hydration_markers(&body);
    assert!(
        clean.contains("30 days") || clean.contains("Upgrade to Flat"),
        "Metrics page should show 30-day limit or upgrade prompt for free users: {clean}"
    );
}

// ---------------------------------------------------------------------------
// Feature: Recent prompts (10)
// ---------------------------------------------------------------------------

/// Test: free user prompt history is limited to 10 most recent prompts.
#[tokio::test]
async fn free_plan_recent_prompts_10() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Clone and push 11 commits with unique AI prompts
    let clone_dest = server.data_dir.path().join("clone-prompts");
    let clone_url = http_clone_url(&client.base_url, "alice", "password123", "alice", "myrepo");
    git_clone_http(&clone_url, &clone_dest);
    init_repo_config(&clone_dest);

    for i in 1..=11 {
        create_commit_with_trailers(
            &clone_dest,
            &format!("file{}.rs", i),
            &format!("fn func{}() {{}}", i),
            &format!("feat: add func{}", i),
            "claude",
            Some("opus"),
            Some(&format!("create function number {}", i)),
            Some(&format!("session-prompts-{}", i)),
            Some(i),
        );
    }
    let push_out = git_push(&clone_dest);
    assert!(
        push_out.status.success(),
        "git push should succeed: {}",
        String::from_utf8_lossy(&push_out.stderr)
    );

    // Allow the post-receive hook to finish indexing trailers
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/prompts", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    let clean = strip_hydration_markers(&body);
    assert!(
        clean.contains("Showing 10 most recent prompts") || clean.contains("Upgrade to Flat"),
        "Prompt history should show 10-limit or upgrade prompt for free users: {clean}"
    );
}

// ---------------------------------------------------------------------------
// Subscription page
// ---------------------------------------------------------------------------

/// Test: subscription page shows correct info for free user.
#[tokio::test]
async fn free_plan_subscription_page() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    let resp = client
        .client
        .get(format!("{}/subscription", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    let body = resp.text().await.unwrap();
    let clean = strip_hydration_markers(&body);

    assert!(
        clean.contains("Free"),
        "Subscription page should show Free plan: {clean}"
    );
    assert!(
        !clean.contains("Manage billing"),
        "Free user should not see Manage billing button: {clean}"
    );
    assert!(
        clean.contains("View plans") || clean.contains("/pricing"),
        "Subscription page should show link to pricing: {clean}"
    );
}
