mod harness;

use harness::{
    TestServer, create_commit, git_clone_http, git_push, http_clone_url, init_repo_config,
};

/// Test: owner can add a collaborator and the collaborator can push.
#[tokio::test]
async fn collaborator_can_push() {
    let server = TestServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    // Alice creates a repo
    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("shared", "shared repo", false).await;

    // Push initial commit
    let clone_url = http_clone_url(&server.base_url, "alice", "password123", "alice", "shared");
    let repo_dir = tmp.path().join("shared");
    git_clone_http(&clone_url, &repo_dir);
    init_repo_config(&repo_dir);
    create_commit(&repo_dir, "init.txt", "init", "Initial");
    git_push(&repo_dir);

    // Bob registers
    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    // Bob can NOT push before being added as collaborator
    let bob_clone_url = http_clone_url(&server.base_url, "bob", "password123", "alice", "shared");
    let bob_dir = tmp.path().join("bob_shared");
    let output = git_clone_http(&bob_clone_url, &bob_dir);
    assert!(
        output.status.success(),
        "bob should be able to clone public repo"
    );
    init_repo_config(&bob_dir);
    create_commit(&bob_dir, "bob.txt", "bob was here", "Bob commit");
    let output = git_push(&bob_dir);
    assert!(
        !output.status.success(),
        "bob should NOT be able to push without collaborator access"
    );

    // Alice adds Bob as collaborator
    alice.add_collaborator("alice", "shared", "bob").await;

    // Bob can NOW push
    let output = git_push(&bob_dir);
    assert!(
        output.status.success(),
        "bob should be able to push as collaborator: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test: owner can remove a collaborator.
#[tokio::test]
async fn remove_collaborator() {
    let server = TestServer::start().await;

    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("myrepo", "test", false).await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    // Add then remove
    let resp = alice.add_collaborator("alice", "myrepo", "bob").await;
    assert!(resp.status().is_success() || resp.status().is_redirection());

    let resp = alice.list_collaborators("alice", "myrepo").await;
    let body = resp.text().await.unwrap();
    assert!(body.contains("bob"), "bob should be listed as collaborator");

    let resp = alice
        .remove_collaborator_by_name("alice", "myrepo", "bob")
        .await;
    assert!(resp.status().is_success() || resp.status().is_redirection());

    let resp = alice.list_collaborators("alice", "myrepo").await;
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("bob") || body.contains("[]"),
        "bob should be removed"
    );
}

/// Test: non-owner cannot add collaborators.
#[tokio::test]
async fn non_owner_cannot_add_collaborator() {
    let server = TestServer::start().await;

    let alice = server.client();
    alice
        .register("alice", "alice@test.com", "password123")
        .await;
    alice.create_repo("private", "test", false).await;

    let bob = server.client();
    bob.register("bob", "bob@test.com", "password123").await;

    let resp = bob.add_collaborator("alice", "private", "bob").await;
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("owner") || body.contains("error") || body.contains("Error"),
        "non-owner should not add collaborators: {body}"
    );
}
