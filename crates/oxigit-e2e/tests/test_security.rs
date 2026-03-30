mod harness;

use harness::*;

/// Test: forged session cookie is rejected.
#[tokio::test]
async fn forged_session_cookie_rejected() {
    let server = TestServer::start().await;

    // Register a real user
    let client = server.client();
    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("secret-repo", "private", true).await;

    // Create a new client with a forged cookie (no HMAC signature)
    let forged_client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Try to list repos with a forged session cookie
    let resp = forged_client
        .post(format!("{}/api/list_repos8263135991273651100", server.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("cookie", "oxigit_session=1:alice")  // forged: no HMAC signature
        .body("")
        .send()
        .await
        .unwrap();

    let body = resp.text().await.unwrap();
    // Should be rejected — either error or empty list (not authenticated)
    assert!(
        body.contains("Not authenticated") || body.contains("error") || body.contains("Error") || body.is_empty(),
        "forged cookie should be rejected, got: {body}"
    );
}

/// Test: forged cookie with wrong signature is rejected.
#[tokio::test]
async fn wrong_signature_cookie_rejected() {
    let server = TestServer::start().await;

    let client = server.client();
    client.register("alice", "alice@test.com", "password123").await;

    let forged_client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Try with a fake signature
    let resp = forged_client
        .post(format!("{}/api/list_repos8263135991273651100", server.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("cookie", "oxigit_session=1:alice:deadbeef0000000000000000000000000000000000000000000000000000000000")
        .body("")
        .send()
        .await
        .unwrap();

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Not authenticated") || body.contains("error") || body.contains("Error") || body.is_empty(),
        "wrong signature should be rejected, got: {body}"
    );
}

/// Test: valid session still works after signing change.
#[tokio::test]
async fn valid_session_works() {
    let server = TestServer::start().await;
    let client = server.client();

    client.register("alice", "alice@test.com", "password123").await;
    client.create_repo("myrepo", "test", false).await;

    // List repos should work with the valid signed session
    let resp = client.list_repos().await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("myrepo"),
        "valid session should work, got: {body}"
    );
}
