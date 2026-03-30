# E2E Test Development Guide

## Prerequisites
E2E tests spawn the actual compiled binary. You MUST build before running:
```
cargo leptos build --release   # or: make build
cargo test -p oxigit-e2e -- --test-threads=4   # or: make e2e (builds + tests)
```

## Test Infrastructure

### TestServer
`TestServer::start()` in `tests/harness/mod.rs`:
- Spawns oxigit-server binary with fresh `TempDir` and random free ports
- Polls `GET /` until server responds (max 15 sec)
- Killed on drop — each test gets an isolated server + database

### TestClient
Wraps `reqwest::Client` with `cookie_store = true` so session cookies persist across requests.
Methods: `register()`, `login()`, `create_repo()`, `fetch_repo_tree()`, `fetch_commits()`,
`create_pr()`, `merge_pr()`, `add_collaborator()`, `list_ssh_keys()`, etc.

## Server Function URL Hashes (Critical Gotcha)
Leptos 0.8 generates server function API paths with a hash based on the module path:
`/api/{fn_name}{hash}` (e.g., `/api/register_user7369397377459021377`).

These hashes are STABLE as long as:
- The function name doesn't change
- The module path doesn't change (moving a function to a different module breaks the hash)
- The crate name doesn't change

**When adding a new server function:**
1. Build the project (`cargo leptos build --release`)
2. Find the generated URL (check browser network tab or server logs)
3. Add a `const API_*` entry in `tests/harness/mod.rs`
4. Add a helper method on `TestClient`

**When moving/renaming a server function:** Update the corresponding `API_*` constant.

## Test Pattern
```rust
#[tokio::test]
async fn test_something() {
    let server = TestServer::start().await;
    let client = server.client();

    // Most tests need an authenticated user
    client.register("alice", "alice@test.com", "password123").await;
    client.login("alice", "password123").await;

    // Create repo, push code, etc.
    client.create_repo("myrepo", "description", false).await;
}
```

## Git Test Helpers
All in `tests/harness/mod.rs`:
- `git_clone_http(url, dest)` / `git_clone_ssh(port, owner, repo, dest, key_path)` — Clone
- `http_clone_url(base_url, user, pass, owner, repo)` — Build URL with embedded Basic Auth
- `create_commit(repo_dir, filename, content, message)` — Create file + add + commit
- `git_push(repo_dir)` / `git_push_ssh(...)` — Push to origin
- `git_pull(repo_dir)` / `git_pull_ssh(...)` — Pull from origin
- `generate_ssh_keypair(dir)` — Generate ed25519 key pair for SSH tests
- `init_repo_config(repo_dir)` — Set user.email/user.name in cloned repo

## SSH Test Pattern
```rust
let (key_path, pub_key) = generate_ssh_keypair(server.data_dir.path());
client.add_ssh_key("mykey", &pub_key).await;
let dest = server.data_dir.path().join("clone-ssh");
git_clone_ssh(server.ssh_port, "alice", "myrepo", &dest, &key_path);
```

## Test File Naming
- Test files: `tests/test_{feature}.rs`
- Harness module: `tests/harness/mod.rs`
