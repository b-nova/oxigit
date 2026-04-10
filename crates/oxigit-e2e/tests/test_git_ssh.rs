mod harness;

use harness::{
    TestServer, create_commit, generate_ssh_keypair, git_clone_ssh, git_pull_ssh, git_push_ssh,
    ssh_available,
};
use std::path::Path;

#[tokio::test]
async fn test_clone_push_pull_ssh() {
    if !ssh_available() {
        eprintln!("SKIP: ssh-keygen not available");
        return;
    }

    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("ssh-repo", "SSH test repo", false).await;

    // Generate SSH keypair and register it
    let key_dir = server.data_dir.path().join("keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    let (key_path, pub_key) = generate_ssh_keypair(&key_dir);

    let resp = client.add_ssh_key("test-key", &pub_key).await;
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "add_ssh_key failed: {}",
        resp.status()
    );

    // Clone via SSH
    let clone1 = server.data_dir.path().join("ssh_clone1");
    let output = git_clone_ssh(server.ssh_port, "alice", "ssh-repo", &clone1, &key_path);
    assert!(
        output.status.success(),
        "ssh clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Create file and push
    init_repo_config(&clone1);
    create_commit(&clone1, "hello.txt", "Hello via SSH!\n", "SSH commit");

    let output = git_push_ssh(&clone1, server.ssh_port, &key_path);
    assert!(
        output.status.success(),
        "ssh push failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Clone to a second directory and verify
    let clone2 = server.data_dir.path().join("ssh_clone2");
    let output = git_clone_ssh(server.ssh_port, "alice", "ssh-repo", &clone2, &key_path);
    assert!(
        output.status.success(),
        "ssh clone2 failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(clone2.join("hello.txt").exists(), "hello.txt should exist");
    let content = std::fs::read_to_string(clone2.join("hello.txt")).unwrap();
    assert_eq!(content, "Hello via SSH!\n");

    // Push another commit, then pull in clone2
    create_commit(&clone1, "second.txt", "More content\n", "Second SSH commit");
    let output = git_push_ssh(&clone1, server.ssh_port, &key_path);
    assert!(output.status.success());

    let output = git_pull_ssh(&clone2, server.ssh_port, &key_path);
    assert!(
        output.status.success(),
        "ssh pull failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        clone2.join("second.txt").exists(),
        "second.txt should exist after pull"
    );
}

#[tokio::test]
async fn test_ssh_push_wrong_user() {
    if !ssh_available() {
        eprintln!("SKIP: ssh-keygen not available");
        return;
    }

    let server = TestServer::start().await;

    // Alice creates a repo
    let alice = server.client();
    let _ = alice
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = alice.create_repo("alice-repo", "Alice's", false).await;

    // Bob registers and adds his SSH key
    let bob = server.client();
    let _ = bob.register("bob", "bob@example.com", "password123").await;

    let key_dir = server.data_dir.path().join("bob_keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    let (bob_key, bob_pub) = generate_ssh_keypair(&key_dir);
    let _ = bob.add_ssh_key("bob-key", &bob_pub).await;

    // Bob clones alice's public repo via SSH
    let clone_dir = server.data_dir.path().join("bob_ssh_clone");
    let output = git_clone_ssh(server.ssh_port, "alice", "alice-repo", &clone_dir, &bob_key);
    assert!(output.status.success());

    init_repo_config(&clone_dir);
    create_commit(&clone_dir, "hack.txt", "pwned\n", "Bad commit");

    // Bob tries to push to alice's repo — should fail
    let output = git_push_ssh(&clone_dir, server.ssh_port, &bob_key);
    assert!(
        !output.status.success(),
        "bob should not be able to push to alice's repo"
    );
}

#[tokio::test]
async fn test_ssh_clone_unregistered_key() {
    if !ssh_available() {
        eprintln!("SKIP: ssh-keygen not available");
        return;
    }

    let server = TestServer::start().await;
    let client = server.client();

    let _ = client
        .register("alice", "alice@example.com", "password123")
        .await;
    let _ = client.create_repo("test-repo", "Test", false).await;

    // Generate a keypair but do NOT register it
    let key_dir = server.data_dir.path().join("unregistered_keys");
    std::fs::create_dir_all(&key_dir).unwrap();
    let (key_path, _pub_key) = generate_ssh_keypair(&key_dir);

    let clone_dir = server.data_dir.path().join("unregistered_clone");
    let output = git_clone_ssh(server.ssh_port, "alice", "test-repo", &clone_dir, &key_path);
    assert!(
        !output.status.success(),
        "clone with unregistered key should fail"
    );
}

fn init_repo_config(repo_dir: &Path) {
    use std::process::Command;
    let _ = Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["checkout", "-b", "main"])
        .current_dir(repo_dir)
        .output();
}
