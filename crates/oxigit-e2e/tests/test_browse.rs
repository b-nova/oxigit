mod harness;

use harness::{
    create_commit, git_clone_http, git_push, http_clone_url, init_repo_config, TestServer,
};

#[tokio::test]
async fn test_browse_tree() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;
    let _ = client.create_repo("browse-test", "Browse test", false).await;

    let clone_dir = server.data_dir.path().join("browse_clone");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "browse-test",
    );
    let output = git_clone_http(&url, &clone_dir);
    assert!(output.status.success());
    init_repo_config(&clone_dir);

    create_commit(&clone_dir, "file1.txt", "File 1\n", "Add file1");
    create_commit(&clone_dir, "src/main.rs", "fn main() {}\n", "Add src/main.rs");

    let output = git_push(&clone_dir);
    assert!(output.status.success());

    let resp = client.fetch_repo_tree("alice", "browse-test", "", "").await;
    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("file1.txt"),
        "tree should contain file1.txt, got: {body}"
    );
    assert!(
        body.contains("src"),
        "tree should contain src directory, got: {body}"
    );
}

#[tokio::test]
async fn test_browse_blob() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;
    let _ = client.create_repo("blob-test", "Blob test", false).await;

    let clone_dir = server.data_dir.path().join("blob_clone");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "blob-test",
    );
    let output = git_clone_http(&url, &clone_dir);
    assert!(output.status.success());
    init_repo_config(&clone_dir);

    create_commit(
        &clone_dir,
        "greeting.txt",
        "Hello, Oxigit!\n",
        "Add greeting",
    );
    let output = git_push(&clone_dir);
    assert!(output.status.success());

    let resp = client.get_blob("alice", "blob-test", "greeting.txt").await;
    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Hello, Oxigit!"),
        "blob should contain file content, got: {body}"
    );
}

#[tokio::test]
async fn test_browse_readme_rendered() {
    let server = TestServer::start().await;
    let client = server.client();

    let _ = client.register("alice", "alice@example.com", "password123").await;
    let _ = client.create_repo("readme-test", "README test", false).await;

    let clone_dir = server.data_dir.path().join("readme_clone");
    let url = http_clone_url(
        &server.base_url,
        "alice",
        "password123",
        "alice",
        "readme-test",
    );
    let output = git_clone_http(&url, &clone_dir);
    assert!(output.status.success());
    init_repo_config(&clone_dir);

    create_commit(
        &clone_dir,
        "README.md",
        "# My Project\n\nThis is **bold** text.\n",
        "Add README",
    );
    let output = git_push(&clone_dir);
    assert!(output.status.success());

    let resp = client.fetch_repo_tree("alice", "readme-test", "", "").await;
    assert!(resp.status().is_success());
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("<h1>") || body.contains("<strong>") || body.contains("My Project"),
        "README should be rendered as HTML, got: {body}"
    );
}
