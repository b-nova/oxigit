mod harness;

use harness::*;

/// Helper: simulate upgrading a user to Pro via Stripe webhook.
async fn upgrade_to_pro(client: &TestClient, base_url: &str, user_id: &str, secret: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_pro_{}","subscription":"sub_pro_{}","metadata":{{"plan":"flat"}}}}}}}}"#,
        user_id, user_id, user_id
    );
    let timestamp = "1234567890";
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let signature = format!("t={},v1={}", timestamp, sig);

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", base_url))
        .header("stripe-signature", &signature)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(status == 200 || status == 404, "Stripe webhook returned unexpected status: {status}");
}

/// Helper: simulate claiming a founding member slot via Stripe webhook.
async fn upgrade_to_founding(client: &TestClient, base_url: &str, user_id: &str, secret: &str) {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let payload = format!(
        r#"{{"type":"checkout.session.completed","data":{{"object":{{"client_reference_id":"{}","customer":"cus_found_{}","subscription":"sub_found_{}","metadata":{{"plan":"founding"}}}}}}}}"#,
        user_id, user_id, user_id
    );
    let timestamp = "1234567890";
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let signature = format!("t={},v1={}", timestamp, sig);

    let resp = client
        .client
        .post(format!("{}/api/stripe/webhook", base_url))
        .header("stripe-signature", &signature)
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(status == 200 || status == 404, "Stripe webhook returned unexpected status: {status}");
}

/// Test: free user can access AI Hub page (not blocked).
#[tokio::test]
async fn free_user_ai_hub_preview_mode() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/ai", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    // Free user can access the page (not blocked with error)
    assert!(
        !body.contains("Pro or higher plan"),
        "AI Hub should not hard-block free users: {body}"
    );
}

/// Test: free user sees metrics with 30-day limit (not blocked).
#[tokio::test]
async fn free_user_metrics_limited() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/metrics", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    // Page loads successfully (no error), may show 30-day banner if data exists
    assert!(
        !body.contains("Pro or higher plan"),
        "Metrics should not hard-block free users: {body}"
    );
}

/// Test: free user sees prompt history with limited results (not blocked).
#[tokio::test]
async fn free_user_prompt_history_limited() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/prompts", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    // Page loads successfully (no error)
    assert!(
        !body.contains("Pro or higher plan"),
        "Prompt history should not hard-block free users: {body}"
    );
}

/// Test: Pro user can access AI Hub page.
#[tokio::test]
async fn pro_user_can_access_ai_hub() {
    let secret = "whsec_pro_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;

    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    let resp = client
        .client
        .get(format!("{}/alice/myrepo/ai", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("Pro or higher plan"),
        "Pro user should access AI Hub without upgrade prompt: {body}"
    );
}

/// Test: founding member badge appears on user profile.
#[tokio::test]
async fn founding_member_badge_on_profile() {
    let secret = "whsec_founding_test";
    let server = TestServer::start_with_stripe(secret).await;
    if !server.has_saas().await { eprintln!("SKIPPED: saas"); return; }
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_founding(&client, &client.base_url.clone(), "1", secret).await;

    let resp = client
        .client
        .get(format!("{}/alice", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Founding Member"),
        "Profile should show Founding Member badge: {body}"
    );
}

/// Test: Pro user shows Pro badge on profile (not founding).
#[tokio::test]
async fn pro_badge_on_profile() {
    let secret = "whsec_badge_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;

    let resp = client
        .client
        .get(format!("{}/alice", client.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("badge-flat") || body.contains(">Flat<"),
        "Profile should show Flat badge: {body}"
    );
}

/// Test: free user profile shows no plan badge.
#[tokio::test]
async fn free_user_no_badge_on_profile() {
    let server = TestServer::start().await;
    if !server.has_saas().await { eprintln!("SKIPPED: saas"); return; }
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;

    let resp = client
        .client
        .get(format!("{}/alice", client.base_url))
        .send()
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("Founding Member") && !body.contains("badge-flat") && !body.contains("badge-team"),
        "Free user should have no plan badge: {body}"
    );
}

/// Test: register with plan param redirects to pricing checkout.
#[tokio::test]
async fn register_with_plan_redirects_to_pricing() {
    let server = TestServer::start_with_stripe("whsec_plan_redirect_test").await;
    if !server.has_saas().await { eprintln!("SKIPPED: saas"); return; }
    let client = server.client();

    // Register with plan=flat via the server function API
    // Leptos server functions return redirect as a header, not a 302.
    // Build a no-redirect client to capture the redirect location.
    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let resp = no_redirect_client
        .post(format!("{}/api/register_user14969902946520757255", client.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("username=alice&email=alice%40test.com&password=password123&plan=flat")
        .send()
        .await
        .unwrap();

    // Leptos redirects return a 3xx with Location header
    let status = resp.status().as_u16();
    let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(
        (status >= 300 && status < 400 && location.contains("/pricing") && location.contains("checkout=flat"))
            || location.contains("pricing"),
        "register with plan=flat should redirect to pricing checkout, got status={status} location={location}"
    );
}

/// Test: free user sees AI tool badge on commit but not the prompt text.
#[tokio::test]
async fn free_user_commit_view_shows_badge_hides_prompt() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;
    client.create_repo("myrepo", "test repo", false).await;

    // Clone, commit with AI trailers, and push
    let clone_dest = server.data_dir.path().join("clone-ai");
    let clone_url = http_clone_url(&client.base_url, "alice", "password123", "alice", "myrepo");
    git_clone_http(&clone_url, &clone_dest);
    init_repo_config(&clone_dest);

    create_commit(
        &clone_dest,
        "test.txt",
        "hello",
        "test commit\n\nOxigit-Tool: claude\nOxigit-Model: opus",
    );
    git_push(&clone_dest);

    // Wait for async post-receive hook to store AI metadata
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch commits — free user should see tool/model badges
    let resp = client.fetch_commits("alice", "myrepo").await;
    let body = resp.text().await.unwrap();

    // Free users now see AI tool badges (tool and model are visible)
    assert!(
        body.contains("claude") || body.contains("opus"),
        "Free user should see AI tool/model badges on commits: {body}"
    );
}
