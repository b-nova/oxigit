use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use tempfile::TempDir;

// Leptos 0.8 appends a hash to server function URLs based on module path.
// These are stable as long as the module structure doesn't change.
const API_REGISTER_USER: &str = "/api/register_user7369397377459021377";
const API_LOGIN_USER: &str = "/api/login_user1990639712270358737";
const API_CREATE_REPO: &str = "/api/create_repo5303270923979939997";
const API_ADD_SSH_KEY: &str = "/api/add_ssh_key6124160161295249415";
const API_DELETE_KEY: &str = "/api/delete_key6124160161295249415";
const API_LIST_SSH_KEYS: &str = "/api/list_ssh_keys6124160161295249415";
const API_LIST_REPOS: &str = "/api/list_repos8263135991273651100";
const API_GET_REPO_TREE: &str = "/api/get_repo_tree13941354728913308461";
const API_GET_BLOB: &str = "/api/get_blob16825275655980583145";

// Phase 7+ server functions
const API_FETCH_REPO_TREE: &str = "/api/fetch_repo_tree13941354728913308461";
const API_FETCH_COMMITS: &str = "/api/fetch_commits9790621075323477947";
const API_FETCH_COMMIT_DIFF: &str = "/api/fetch_commit_diff14048240763101790269";
const API_FETCH_USER_PROFILE: &str = "/api/fetch_user_profile10576369475516542132";
const API_FORK_REPO: &str = "/api/fork_repo13941354728913308461";
// Issues
const API_CREATE_ISSUE: &str = "/api/create_issue18200392593341856158";
const API_LIST_ISSUES: &str = "/api/list_issues14820508467033819727";
const API_GET_ISSUE: &str = "/api/get_issue17359750668367926196";
const API_CLOSE_ISSUE: &str = "/api/close_issue_action17359750668367926196";
const API_REOPEN_ISSUE: &str = "/api/reopen_issue_action17359750668367926196";
const API_ADD_COMMENT: &str = "/api/add_comment17359750668367926196";

// Explore
const API_EXPLORE_REPOS: &str = "/api/explore_repos1960154575694798443";

// Pull Request endpoints
const API_LIST_PRS: &str = "/api/list_prs10190582681436013828";
const API_CREATE_PR: &str = "/api/create_pr2627984678080233679";
const API_GET_PR: &str = "/api/get_pr846130105414292458";
const API_MERGE_PR: &str = "/api/merge_pr846130105414292458";
const API_CLOSE_PR: &str = "/api/close_pr846130105414292458";

const API_ADD_COLLABORATOR: &str = "/api/add_collaborator2090155042817890545";
const API_REMOVE_COLLABORATOR: &str = "/api/remove_collaborator2090155042817890545";
const API_LIST_COLLABORATORS: &str = "/api/list_collaborators2090155042817890545";

// AI Timeline
const API_FETCH_AI_TIMELINE: &str = "/api/fetch_ai_timeline16115729671913729000";
const API_ATTACH_AI_METADATA: &str = "/api/attach_ai_metadata14048240763101790269";

/// A running server instance with its own data directory and ports.
pub struct TestServer {
    pub http_port: u16,
    pub ssh_port: u16,
    pub base_url: String,
    pub data_dir: TempDir,
    child: Child,
}

impl TestServer {
    pub async fn start() -> Self {
        let http_port = free_port();
        let ssh_port = free_port();
        let data_dir = TempDir::new().expect("failed to create temp dir");

        let binary = find_binary();

        let child = Command::new(&binary)
            .env("OXIGIT_DATA_DIR", data_dir.path())
            .env("OXIGIT_HTTP_ADDR", format!("127.0.0.1:{http_port}"))
            .env("OXIGIT_SSH_ADDR", format!("127.0.0.1:{ssh_port}"))
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| {
                panic!(
                    "failed to start server binary at {}: {e}",
                    binary.display()
                )
            });

        let base_url = format!("http://127.0.0.1:{http_port}");

        let server = TestServer {
            http_port,
            ssh_port,
            base_url,
            data_dir,
            child,
        };

        // Wait for server to be ready
        let client = reqwest::Client::new();
        for i in 0..150 {
            if client.get(&server.base_url).send().await.is_ok() {
                return server;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if i == 149 {
                panic!("server did not become ready within 15 seconds");
            }
        }

        server
    }

    pub fn client(&self) -> TestClient {
        TestClient::new(&self.base_url)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind to free port");
    listener.local_addr().unwrap().port()
}

fn find_binary() -> PathBuf {
    if let Ok(path) = std::env::var("OXIGIT_BINARY") {
        let p = PathBuf::from(path);
        assert!(p.exists(), "OXIGIT_BINARY does not exist: {}", p.display());
        return p;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("failed to find workspace root")
        .to_path_buf();

    let candidates = [
        workspace_root.join("target/server-release/oxigit-server"),
        workspace_root.join("target/release/oxigit-server"),
        workspace_root.join("target/debug/oxigit-server"),
    ];

    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }

    panic!(
        "Could not find oxigit-server binary. Run `cargo leptos build --release` first, \
         or set OXIGIT_BINARY env var. Searched: {:?}",
        candidates
    );
}

// ---------------------------------------------------------------------------
// TestClient
// ---------------------------------------------------------------------------

pub struct TestClient {
    pub client: reqwest::Client,
    pub base_url: String,
}

impl TestClient {
    pub fn new(base_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build reqwest client");
        TestClient {
            client,
            base_url: base_url.to_string(),
        }
    }

    pub async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_REGISTER_USER))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "username={}&email={}&password={}",
                urlencoded(username),
                urlencoded(email),
                urlencoded(password)
            ))
            .send()
            .await
            .expect("register request failed")
    }

    pub async fn login(&self, username: &str, password: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LOGIN_USER))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "username={}&password={}",
                urlencoded(username),
                urlencoded(password)
            ))
            .send()
            .await
            .expect("login request failed")
    }

    pub async fn create_repo(
        &self,
        name: &str,
        description: &str,
        is_private: bool,
    ) -> reqwest::Response {
        let body = format!(
            "name={}&description={}&is_private={}",
            urlencoded(name),
            urlencoded(description),
            is_private
        );
        self.client
            .post(format!("{}{}", self.base_url, API_CREATE_REPO))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("create_repo request failed")
    }

    pub async fn add_ssh_key(&self, name: &str, public_key: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_ADD_SSH_KEY))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "name={}&public_key={}",
                urlencoded(name),
                urlencoded(public_key)
            ))
            .send()
            .await
            .expect("add_ssh_key request failed")
    }

    pub async fn delete_ssh_key(&self, key_id: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_DELETE_KEY))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("key_id={key_id}"))
            .send()
            .await
            .expect("delete_key request failed")
    }

    pub async fn list_ssh_keys(&self) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_SSH_KEYS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("")
            .send()
            .await
            .expect("list_ssh_keys request failed")
    }

    pub async fn list_repos(&self) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_REPOS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("")
            .send()
            .await
            .expect("list_repos request failed")
    }

    pub async fn get_repo_tree(
        &self,
        owner: &str,
        repo: &str,
        git_ref: &str,
        path: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_REPO_TREE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&git_ref={}&path={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(git_ref),
                urlencoded(path)
            ))
            .send()
            .await
            .expect("get_repo_tree request failed")
    }

    pub async fn get_blob(&self, owner: &str, repo: &str, path: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_BLOB))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&path={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(path)
            ))
            .send()
            .await
            .expect("get_blob request failed")
    }

    pub async fn fetch_repo_tree(
        &self,
        owner: &str,
        repo: &str,
        git_ref: &str,
        path: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_REPO_TREE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&git_ref={}&path={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(git_ref),
                urlencoded(path)
            ))
            .send()
            .await
            .expect("fetch_repo_tree request failed")
    }

    pub async fn fetch_commits(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_COMMITS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("fetch_commits request failed")
    }

    pub async fn fetch_commit_diff(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_COMMIT_DIFF))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&sha={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(sha)
            ))
            .send()
            .await
            .expect("fetch_commit_diff request failed")
    }

    pub async fn fetch_user_profile(&self, username: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_USER_PROFILE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("username={}", urlencoded(username)))
            .send()
            .await
            .expect("fetch_user_profile request failed")
    }

    pub async fn fork_repo(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FORK_REPO))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("fork_repo request failed")
    }

    pub async fn create_issue(&self, owner: &str, repo: &str, title: &str, description: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CREATE_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("owner={}&repo={}&title={}&description={}", urlencoded(owner), urlencoded(repo), urlencoded(title), urlencoded(description)))
            .send().await.expect("create_issue failed")
    }

    pub async fn list_issues(&self, owner: &str, repo: &str, status: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_ISSUES))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("owner={}&repo={}&status={}", urlencoded(owner), urlencoded(repo), urlencoded(status)))
            .send().await.expect("list_issues failed")
    }

    pub async fn get_issue(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("owner={}&repo={}&number={}", urlencoded(owner), urlencoded(repo), number))
            .send().await.expect("get_issue failed")
    }

    pub async fn close_issue(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CLOSE_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("owner={}&repo={}&number={}", urlencoded(owner), urlencoded(repo), number))
            .send().await.expect("close_issue failed")
    }

    pub async fn reopen_issue(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_REOPEN_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("owner={}&repo={}&number={}", urlencoded(owner), urlencoded(repo), number))
            .send().await.expect("reopen_issue failed")
    }

    pub async fn add_issue_comment(&self, owner: &str, repo: &str, number: i64, body: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_ADD_COMMENT))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("owner={}&repo={}&number={}&body={}", urlencoded(owner), urlencoded(repo), number, urlencoded(body)))
            .send().await.expect("add_comment failed")
    }

    pub async fn explore_repos(&self, query: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_EXPLORE_REPOS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("query={}", urlencoded(query)))
            .send()
            .await
            .expect("explore_repos request failed")
    }

    pub async fn create_pr(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        description: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CREATE_PR))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&title={}&description={}&source_branch={}&target_branch={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(title),
                urlencoded(description),
                urlencoded(source_branch),
                urlencoded(target_branch),
            ))
            .send()
            .await
            .expect("create_pr request failed")
    }

    pub async fn list_prs(&self, owner: &str, repo: &str, status: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_PRS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&status={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(status),
            ))
            .send()
            .await
            .expect("list_prs request failed")
    }

    pub async fn get_pr(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_PR))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}",
                urlencoded(owner),
                urlencoded(repo),
                number,
            ))
            .send()
            .await
            .expect("get_pr request failed")
    }

    pub async fn merge_pr(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_MERGE_PR))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}",
                urlencoded(owner),
                urlencoded(repo),
                number,
            ))
            .send()
            .await
            .expect("merge_pr request failed")
    }

    pub async fn close_pr(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CLOSE_PR))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}",
                urlencoded(owner),
                urlencoded(repo),
                number,
            ))
            .send()
            .await
            .expect("close_pr request failed")
    }

    pub async fn add_collaborator(&self, owner: &str, repo: &str, username: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_ADD_COLLABORATOR))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&username={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(username)
            ))
            .send()
            .await
            .expect("add_collaborator request failed")
    }

    pub async fn remove_collaborator_by_name(&self, owner: &str, repo: &str, username: &str) -> reqwest::Response {
        // First list to find user_id, then remove
        let resp = self.list_collaborators(owner, repo).await;
        let body = resp.text().await.unwrap();
        // Extract user_id for the given username from the response
        // Simple approach: find the user_id near the username in the JSON
        let user_id = body
            .split("user_id")
            .skip(1)
            .find_map(|chunk| {
                if chunk.contains(username) || body.split(username).nth(1).map_or(false, |_| true) {
                    chunk.split(|c: char| !c.is_ascii_digit())
                        .find(|s| !s.is_empty())
                        .and_then(|s| s.parse::<i64>().ok())
                }
                else { None }
            })
            .unwrap_or(0);

        self.client
            .post(format!("{}{}", self.base_url, API_REMOVE_COLLABORATOR))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&user_id={}",
                urlencoded(owner),
                urlencoded(repo),
                user_id
            ))
            .send()
            .await
            .expect("remove_collaborator request failed")
    }

    pub async fn list_collaborators(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_COLLABORATORS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("list_collaborators request failed")
    }

    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .expect("GET request failed")
    }

    pub async fn fetch_ai_timeline(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_AI_TIMELINE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("fetch_ai_timeline request failed")
    }

    pub async fn attach_ai_metadata(
        &self,
        owner: &str,
        repo: &str,
        commit_sha: &str,
        ai_tool: &str,
        ai_model: Option<&str>,
        ai_prompt: Option<&str>,
        ai_session_id: Option<&str>,
        ai_files_touched: Option<&str>,
    ) -> reqwest::Response {
        let mut body = format!(
            "owner={}&repo={}&commit_sha={}&ai_tool={}",
            urlencoded(owner),
            urlencoded(repo),
            urlencoded(commit_sha),
            urlencoded(ai_tool)
        );
        if let Some(model) = ai_model {
            body.push_str(&format!("&ai_model={}", urlencoded(model)));
        }
        if let Some(prompt) = ai_prompt {
            body.push_str(&format!("&ai_prompt={}", urlencoded(prompt)));
        }
        if let Some(session) = ai_session_id {
            body.push_str(&format!("&ai_session_id={}", urlencoded(session)));
        }
        if let Some(files) = ai_files_touched {
            body.push_str(&format!("&ai_files_touched={}", urlencoded(files)));
        }
        self.client
            .post(format!("{}{}", self.base_url, API_ATTACH_AI_METADATA))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("attach_ai_metadata request failed")
    }
}

fn urlencoded(s: &str) -> String {
    // Minimal percent-encoding for form values
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('+', "%2B")
        .replace(' ', "+")
        .replace('@', "%40")
}

// ---------------------------------------------------------------------------
// Git helpers
// ---------------------------------------------------------------------------

/// Clone a repo via HTTP with optional basic auth credentials.
pub fn git_clone_http(url: &str, dest: &Path) -> Output {
    Command::new("git")
        .args(["clone", url, dest.to_str().unwrap()])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git clone failed to execute")
}

/// Clone a repo via SSH.
pub fn git_clone_ssh(
    port: u16,
    owner: &str,
    repo: &str,
    dest: &Path,
    key_path: &Path,
) -> Output {
    let url = format!("ssh://git@127.0.0.1:{port}/{owner}/{repo}.git");
    let ssh_cmd = format!(
        "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p {port}",
        key_path.display()
    );
    Command::new("git")
        .args(["clone", &url, dest.to_str().unwrap()])
        .env("GIT_SSH_COMMAND", ssh_cmd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git clone ssh failed to execute")
}

/// Push from a repo directory.
pub fn git_push(repo_dir: &Path) -> Output {
    Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git push failed to execute")
}

/// Push from a repo directory via SSH.
pub fn git_push_ssh(repo_dir: &Path, port: u16, key_path: &Path) -> Output {
    let ssh_cmd = format!(
        "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p {port}",
        key_path.display()
    );
    Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_dir)
        .env("GIT_SSH_COMMAND", ssh_cmd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git push ssh failed to execute")
}

/// Pull in a repo directory.
pub fn git_pull(repo_dir: &Path) -> Output {
    Command::new("git")
        .args(["pull", "origin", "main"])
        .current_dir(repo_dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git pull failed to execute")
}

/// Pull in a repo directory via SSH.
pub fn git_pull_ssh(repo_dir: &Path, port: u16, key_path: &Path) -> Output {
    let ssh_cmd = format!(
        "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p {port}",
        key_path.display()
    );
    Command::new("git")
        .args(["pull", "origin", "main"])
        .current_dir(repo_dir)
        .env("GIT_SSH_COMMAND", ssh_cmd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("git pull ssh failed to execute")
}

/// Create a file, add, and commit in a repo directory.
pub fn create_commit(repo_dir: &Path, filename: &str, content: &str, message: &str) {
    let file_path = repo_dir.join(filename);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create parent dirs");
    }
    std::fs::write(&file_path, content).expect("failed to write file");

    let status = Command::new("git")
        .args(["add", filename])
        .current_dir(repo_dir)
        .output()
        .expect("git add failed");
    assert!(status.status.success(), "git add failed");

    let status = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_dir)
        .env("GIT_AUTHOR_NAME", "Test User")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test User")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git commit failed");
    assert!(
        status.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// Create a file, add, and commit with AI trailers in the commit message.
pub fn create_commit_with_ai_trailers(
    repo_dir: &Path,
    filename: &str,
    content: &str,
    subject: &str,
    ai_tool: &str,
    ai_model: Option<&str>,
    ai_prompt: Option<&str>,
    ai_session: Option<&str>,
) {
    let file_path = repo_dir.join(filename);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create parent dirs");
    }
    std::fs::write(&file_path, content).expect("failed to write file");

    let status = Command::new("git")
        .args(["add", filename])
        .current_dir(repo_dir)
        .output()
        .expect("git add failed");
    assert!(status.status.success(), "git add failed");

    // Build commit message with trailers
    let mut message = format!("{}\n\n", subject);
    message.push_str(&format!("AI-Tool: {}\n", ai_tool));
    if let Some(model) = ai_model {
        message.push_str(&format!("AI-Model: {}\n", model));
    }
    if let Some(prompt) = ai_prompt {
        message.push_str(&format!("AI-Prompt: {}\n", prompt));
    }
    if let Some(session) = ai_session {
        message.push_str(&format!("AI-Session: {}\n", session));
    }

    let status = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(repo_dir)
        .env("GIT_AUTHOR_NAME", "Test User")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test User")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git commit failed");
    assert!(
        status.status.success(),
        "git commit with trailers failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// Get the HEAD commit SHA from a repo directory.
pub fn get_head_sha(repo_dir: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .expect("git rev-parse failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Generate an ed25519 SSH keypair in the given directory.
/// Returns (private_key_path, public_key_string).
pub fn generate_ssh_keypair(dir: &Path) -> (PathBuf, String) {
    let key_path = dir.join("test_key");
    let status = Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-f",
            key_path.to_str().unwrap(),
            "-N",
            "",
            "-q",
        ])
        .output()
        .expect("ssh-keygen failed to execute");
    assert!(
        status.status.success(),
        "ssh-keygen failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let pub_key =
        std::fs::read_to_string(key_path.with_extension("pub")).expect("failed to read public key");
    (key_path, pub_key.trim().to_string())
}

/// Check if SSH tools are available on this system.
pub fn ssh_available() -> bool {
    Command::new("ssh-keygen")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Initialize git config in a repo directory (for cloned repos).
pub fn init_repo_config(repo_dir: &Path) {
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_dir)
        .output()
        .expect("git config email failed");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_dir)
        .output()
        .expect("git config name failed");
    // Ensure we're on main
    let _ = Command::new("git")
        .args(["checkout", "-b", "main"])
        .current_dir(repo_dir)
        .output();
}

/// Build a clone URL with embedded HTTP basic auth credentials.
pub fn http_clone_url(base_url: &str, user: &str, pass: &str, owner: &str, repo: &str) -> String {
    let without_scheme = base_url.strip_prefix("http://").unwrap();
    format!("http://{user}:{pass}@{without_scheme}/{owner}/{repo}.git")
}

/// Build a clone URL without credentials.
pub fn http_clone_url_no_auth(base_url: &str, owner: &str, repo: &str) -> String {
    format!("{base_url}/{owner}/{repo}.git")
}
