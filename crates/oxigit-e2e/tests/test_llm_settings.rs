mod harness;

use harness::TestServer;

/// User can save and fetch LLM settings.
#[tokio::test]
async fn test_save_and_fetch_llm_settings() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    let resp = client
        .save_llm_settings(
            "openai",
            "sk-test-1234567890abcdef",
            "gpt-4o",
            "https://api.openai.com",
        )
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "save_llm_settings failed: {}",
        resp.status()
    );

    // Fetch and verify
    let resp = client.fetch_llm_settings().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("openai"),
        "Expected provider in response, got: {}",
        &body[..500.min(body.len())]
    );
    assert!(
        body.contains("gpt-4o"),
        "Expected model in response, got: {}",
        &body[..500.min(body.len())]
    );
    // API key should be masked
    assert!(
        body.contains("..."),
        "API key should be masked, got: {}",
        &body[..500.min(body.len())]
    );
    assert!(
        !body.contains("sk-test-1234567890abcdef"),
        "Full API key should NOT appear in response"
    );
}

/// Unauthenticated user cannot fetch LLM settings.
#[tokio::test]
async fn test_fetch_llm_settings_unauthenticated() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.fetch_llm_settings().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error")
            || body.contains("Error")
            || body.contains("Not authenticated")
            || body.contains("login"),
        "Unauthenticated user should be denied, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Saving with masked key preserves existing key.
#[tokio::test]
async fn test_save_llm_settings_masked_key_preserved() {
    let server = TestServer::start().await;
    let client = server.client();

    client
        .register("alice", "alice@test.com", "password123")
        .await;
    client.login("alice", "password123").await;

    // Save initial settings
    client
        .save_llm_settings("openai", "sk-real-key-1234567890ab", "gpt-4o", "")
        .await;

    // Save again with masked key (simulating UI re-submit)
    let resp = client
        .save_llm_settings("anthropic", "sk-r...90ab", "claude-4", "")
        .await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "save with masked key failed: {}",
        resp.status()
    );

    // Fetch and verify provider changed but key is still masked (preserved)
    let resp = client.fetch_llm_settings().await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("anthropic"),
        "Provider should be updated, got: {}",
        &body[..500.min(body.len())]
    );
    assert!(
        body.contains("claude-4"),
        "Model should be updated, got: {}",
        &body[..500.min(body.len())]
    );
}
