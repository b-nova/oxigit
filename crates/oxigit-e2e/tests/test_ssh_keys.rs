mod harness;

use harness::{generate_ssh_keypair, ssh_available, TestServer};

#[tokio::test]
async fn test_add_and_list_ssh_keys() {
    if !ssh_available() {
        eprintln!("SKIP: ssh-keygen not available");
        return;
    }

    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;

    let key_dir = server.data_dir.path().join("keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    let (_key_path, pub_key) = generate_ssh_keypair(&key_dir);

    let resp = client.add_ssh_key("my-laptop", &pub_key).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "add_ssh_key failed: {}",
        resp.status()
    );

    let resp = client.list_ssh_keys().await;
    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("my-laptop") || body.contains("SHA256"),
        "expected key info in response, got: {body}"
    );
}

#[tokio::test]
async fn test_add_duplicate_ssh_key() {
    if !ssh_available() {
        eprintln!("SKIP: ssh-keygen not available");
        return;
    }

    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;

    let key_dir = server.data_dir.path().join("keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    let (_key_path, pub_key) = generate_ssh_keypair(&key_dir);

    let _ = client.add_ssh_key("key1", &pub_key).await;

    // Adding the same key again should fail (duplicate fingerprint)
    let resp = client.add_ssh_key("key2", &pub_key).await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error") || body.contains("Error") || body.contains("already") || body.contains("UNIQUE"),
        "expected duplicate key error, got: {body}"
    );
}

#[tokio::test]
async fn test_delete_ssh_key() {
    if !ssh_available() {
        eprintln!("SKIP: ssh-keygen not available");
        return;
    }

    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;

    let key_dir = server.data_dir.path().join("keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    let (_key_path, pub_key) = generate_ssh_keypair(&key_dir);

    let _ = client.add_ssh_key("to-delete", &pub_key).await;

    // Delete the key (key_id = 1, since it's the first key added)
    let resp = client.delete_ssh_key(1).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "delete_key failed: {}",
        resp.status()
    );

    // List keys — should be empty
    let resp = client.list_ssh_keys().await;
    let body = resp.text().await.unwrap();
    // The response should not contain the key name anymore
    // (or return an empty list)
    assert!(
        !body.contains("to-delete") || body.contains("[]"),
        "key should have been deleted, got: {body}"
    );
}

#[tokio::test]
async fn test_add_invalid_ssh_key() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;

    let resp = client.add_ssh_key("bad-key", "not-a-valid-key").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("error") || body.contains("Error") || body.contains("invalid") || body.contains("Invalid"),
        "expected invalid key error, got: {body}"
    );
}
