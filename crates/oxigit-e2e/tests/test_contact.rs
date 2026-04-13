mod harness;

use harness::TestServer;

/// Submit a valid contact inquiry succeeds.
#[tokio::test]
async fn test_submit_contact_inquiry() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client
        .submit_contact_inquiry(
            "Jane Doe",
            "jane@example.com",
            "ACME Corp",
            "Interested in Oxigit for our team.",
        )
        .await;

    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "Contact inquiry should succeed, got: {}",
        resp.status()
    );
}

/// Submit with missing required fields fails.
#[tokio::test]
async fn test_submit_contact_inquiry_missing_fields() {
    let server = TestServer::start().await;
    let client = server.client();

    // Empty name
    let resp = client
        .submit_contact_inquiry("", "jane@example.com", "", "Hello")
        .await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error") || body.contains("Error") || body.contains("required"),
        "Expected validation error for missing name, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Submit with missing message fails.
#[tokio::test]
async fn test_submit_contact_inquiry_missing_message() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client
        .submit_contact_inquiry("Jane", "jane@example.com", "", "")
        .await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("error") || body.contains("Error") || body.contains("required"),
        "Expected validation error for missing message, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Contact page renders via SSR GET.
#[tokio::test]
async fn test_contact_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.get("/contact").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Contact") || body.contains("contact"),
        "Expected contact page, got: {}",
        &body[..500.min(body.len())]
    );
}

/// Contact inquiry shows up in admin dashboard.
#[tokio::test]
async fn test_contact_inquiry_visible_in_admin() {
    let server = TestServer::start().await;
    let client = server.client();

    // First user is admin
    client
        .register("admin", "admin@test.com", "password123")
        .await;
    client.login("admin", "password123").await;

    // Submit a contact inquiry (no auth needed)
    let client2 = server.client();
    client2
        .submit_contact_inquiry(
            "Jane Doe",
            "jane@example.com",
            "ACME",
            "Hello from E2E test",
        )
        .await;

    // Admin should see it in dashboard
    let resp = client.get_admin_dashboard().await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Jane Doe") || body.contains("jane@example.com"),
        "Admin dashboard should show contact inquiry, got: {}",
        &body[..500.min(body.len())]
    );
}
