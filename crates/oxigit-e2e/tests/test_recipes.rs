mod harness;

use harness::*;

/// Helper: simulate upgrading a user to Pro (flat) plan via Stripe webhook.
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

/// Test that the recipe marketplace page loads.
#[tokio::test]
async fn test_marketplace_page_loads() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.get("/recipes").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Recipe Marketplace"), "Expected marketplace heading, got: {}", &body[..300.min(body.len())]);
    assert!(body.contains("No recipes found") || body.contains("recipe"), "Expected empty state or recipes");
}

/// Test that the session detail page shows "Share as Recipe" button for owner.
#[tokio::test]
async fn test_session_detail_shows_share_button() {
    let secret = "whsec_recipe_share_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("alice", "password123").await;
    client.create_repo("reciperepo", "Recipe test", false).await;

    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "reciperepo");
    let dest = server.data_dir.path().join("clone-recipe");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "app.rs", "fn main() {}", "feat: main",
        "claude-code", Some("claude-opus-4-6"), Some("Create main function"),
        Some("recipe-sess-1"), Some(1),
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/alice/reciperepo/ai/recipe-sess-1").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Share as Recipe"), "Expected 'Share as Recipe' button on session detail, got: {}", &body[..500.min(body.len())]);
}

/// Test that the share recipe page loads.
#[tokio::test]
async fn test_share_recipe_page_loads() {
    let secret = "whsec_recipe_share_page_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("bob", "bob@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("bob", "password123").await;
    client.create_repo("sharerepo", "Share test", false).await;

    let clone_url = http_clone_url(&server.base_url, "bob", "password123", "bob", "sharerepo");
    let dest = server.data_dir.path().join("clone-share");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "lib.rs", "fn lib() {}", "feat: lib",
        "cursor", None, Some("Create library"),
        Some("share-sess-1"), None,
    );

    let push = git_push(&dest);
    assert!(push.status.success());

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client.get("/bob/sharerepo/ai/share-sess-1/share").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Share Session as Recipe") || body.contains("Share Recipe"),
        "Expected share recipe form, got: {}", &body[..300.min(body.len())]);
}

/// Test that the Recipes link appears in the navbar.
#[tokio::test]
async fn test_navbar_has_recipes_link() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.get("/").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("Recipes"), "Expected 'Recipes' link in navbar");
    assert!(body.contains("/recipes"), "Expected /recipes href in navbar");
}

/// Test that marketplace shows no recipes for an empty instance.
#[tokio::test]
async fn test_marketplace_empty_state() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("carol", "carol@test.com", "password123").await;
    client.login("carol", "password123").await;

    let resp = client.get("/recipes").await;
    let body = resp.text().await.unwrap();

    assert!(body.contains("No recipes found"), "Expected empty marketplace message");
}

/// Test that non-owner does not see share button.
#[tokio::test]
async fn test_share_button_hidden_for_non_owner() {
    let secret = "whsec_recipe_nonowner_test";
    let server = TestServer::start_with_stripe(secret).await;
    let client = server.client();

    client.register("dave", "dave@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "1", secret).await;
    client.login("dave", "password123").await;
    client.create_repo("owneronly", "Owner only", false).await;

    let clone_url = http_clone_url(&server.base_url, "dave", "password123", "dave", "owneronly");
    let dest = server.data_dir.path().join("clone-owneronly");
    git_clone_http(&clone_url, &dest);
    init_repo_config(&dest);

    create_commit_with_trailers(
        &dest, "x.rs", "fn x() {}", "feat: x",
        "claude-code", None, Some("Build x"),
        Some("owner-sess-1"), None,
    );
    git_push(&dest);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Login as different user — also needs pro plan to access AI session detail
    client.register("eve", "eve@test.com", "password123").await;
    upgrade_to_pro(&client, &client.base_url.clone(), "2", secret).await;
    client.login("eve", "password123").await;

    let resp = client.get("/dave/owneronly/ai/owner-sess-1").await;
    let body = resp.text().await.unwrap();

    // Non-owner should NOT see the share button
    assert!(!body.contains("Share as Recipe"), "Non-owner should not see share button");
}
