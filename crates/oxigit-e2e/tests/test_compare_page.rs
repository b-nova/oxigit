mod harness;

use harness::TestServer;

/// Compare page renders with the comparison table.
#[tokio::test]
async fn test_compare_page_renders() {
    let server = TestServer::start().await;
    let client = server.client();

    let resp = client.get("/compare").await;
    let body = resp.text().await.unwrap();

    assert!(
        body.contains("Oxigit vs"),
        "Expected comparison title, got: {}",
        &body[..500.min(body.len())]
    );
    assert!(body.contains("Gitea"), "Expected Gitea in comparison table");
    assert!(
        body.contains("Forgejo"),
        "Expected Forgejo in comparison table"
    );
    assert!(
        body.contains("GitHub"),
        "Expected GitHub in comparison table"
    );
}
