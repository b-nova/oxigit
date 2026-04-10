#![allow(dead_code, clippy::too_many_arguments)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use tempfile::TempDir;

// Leptos 0.8 appends a hash to server function URLs based on module path.
// These are stable as long as the module structure doesn't change.
const API_REGISTER_USER: &str = "/api/register_user14969902946520757255";
const API_LOGIN_USER: &str = "/api/login_user9081721409912083587";
const API_CREATE_REPO: &str = "/api/create_repo1862433574170527599";
const API_ADD_SSH_KEY: &str = "/api/add_ssh_key18273532019570338376";
const API_DELETE_KEY: &str = "/api/delete_key18273532019570338376";
const API_LIST_SSH_KEYS: &str = "/api/list_ssh_keys18273532019570338376";
const API_LIST_REPOS: &str = "/api/list_repos2931821442575655280";
const API_GET_REPO_TREE: &str = "/api/get_repo_tree12474197787027312135";
const API_GET_BLOB: &str = "/api/get_blob1012343709035136737";

// Phase 7+ server functions
const API_FETCH_REPO_TREE: &str = "/api/fetch_repo_tree12474197787027312135";
const API_FETCH_COMMITS: &str = "/api/fetch_commits14035443652913035783";
const API_FETCH_COMMIT_DIFF: &str = "/api/fetch_commit_diff7316923189440040834";
const API_FETCH_USER_PROFILE: &str = "/api/fetch_user_profile8625115204815136243";
const API_FORK_REPO: &str = "/api/fork_repo12474197787027312135";
// Issues
const API_CREATE_ISSUE: &str = "/api/create_issue12346134817373895594";
const API_LIST_ISSUES: &str = "/api/list_issues16345326029088989429";
const API_GET_ISSUE: &str = "/api/get_issue516512015013301334";
const API_CLOSE_ISSUE: &str = "/api/close_issue_action516512015013301334";
const API_REOPEN_ISSUE: &str = "/api/reopen_issue_action516512015013301334";
const API_ADD_COMMENT: &str = "/api/add_comment516512015013301334";

// Explore
const API_EXPLORE_REPOS: &str = "/api/explore_repos8670121273841898870";

// Pull Request endpoints
const API_LIST_PRS: &str = "/api/list_prs7737783772562051914";
const API_CREATE_PR: &str = "/api/create_pr10776610590872517659";
const API_GET_PR: &str = "/api/get_pr9549500000915392773";
const API_MERGE_PR: &str = "/api/merge_pr9549500000915392773";
const API_CLOSE_PR: &str = "/api/close_pr9549500000915392773";

const API_ADD_COLLABORATOR: &str = "/api/add_collaborator2543744637116902123";
const API_REMOVE_COLLABORATOR: &str = "/api/remove_collaborator2543744637116902123";
const API_LIST_COLLABORATORS: &str = "/api/list_collaborators2543744637116902123";

// AI Timeline
const API_FETCH_AI_TIMELINE: &str = "/api/fetch_ai_timeline4144953925164700098";
const API_ATTACH_AI_METADATA: &str = "/api/attach_ai_metadata7316923189440040834";
const API_INSTALL_AI_HOOK: &str = "/api/install_ai_hook2543744637116902123";
const API_CHECK_INSTALLED_HOOKS: &str = "/api/check_installed_hooks2543744637116902123";

// Smart Diff Review
const API_GET_DIFF_REVIEW: &str = "/api/get_diff_review7316923189440040834";
const API_GENERATE_DIFF_SUMMARY: &str = "/api/generate_diff_summary7316923189440040834";

// Remix
const API_FETCH_REMIX_GUIDE: &str = "/api/fetch_remix_guide12442678095498596403";

// Pricing
const API_FETCH_PRICING_INFO: &str = "/api/fetch_pricing_info17567552309871395660";

// Webhooks (Deploy Previews)
const API_LIST_REPO_WEBHOOKS: &str = "/api/list_repo_webhooks2543744637116902123";
const API_ADD_WEBHOOK: &str = "/api/add_webhook2543744637116902123";
const API_DELETE_WEBHOOK: &str = "/api/delete_webhook2543744637116902123";

// Session/Prompt Revert
const API_REVERT_SESSION: &str = "/api/revert_session3910524453944931178";

// Organizations (Team Features)
const API_CREATE_ORG: &str = "/api/create_org7865708971083396342";
const API_GET_ORG_SETTINGS: &str = "/api/get_org_settings4903785691938244665";
const API_ADD_MEMBER: &str = "/api/add_member4903785691938244665";
const API_REMOVE_MEMBER: &str = "/api/remove_member4903785691938244665";
const API_LIST_MY_ORGS: &str = "/api/list_my_orgs18378147844618610829";
const API_SWITCH_ORG: &str = "/api/switch_org18378147844618610829";

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
                panic!("failed to start server binary at {}: {e}", binary.display())
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

    /// Start a server with Stripe webhook secret configured for billing tests.
    pub async fn start_with_stripe(webhook_secret: &str) -> Self {
        let http_port = free_port();
        let ssh_port = free_port();
        let data_dir = TempDir::new().expect("failed to create temp dir");

        let binary = find_binary();

        let child = Command::new(&binary)
            .env("OXIGIT_DATA_DIR", data_dir.path())
            .env("OXIGIT_HTTP_ADDR", format!("127.0.0.1:{http_port}"))
            .env("OXIGIT_SSH_ADDR", format!("127.0.0.1:{ssh_port}"))
            .env("STRIPE_WEBHOOK_SECRET", webhook_secret)
            .env("STRIPE_SECRET_KEY", "sk_test_fake")
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| {
                panic!("failed to start server binary at {}: {e}", binary.display())
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

    /// Check if the server was built with SaaS features (Stripe webhook route exists).
    pub async fn has_saas(&self) -> bool {
        let client = reqwest::Client::new();
        // OPTIONS or POST to the webhook — a 404 means the route doesn't exist
        let resp = client
            .post(format!("{}/api/stripe/webhook", self.base_url))
            .send()
            .await;
        match resp {
            Ok(r) => r.status().as_u16() != 404,
            Err(_) => false,
        }
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

    pub async fn register(&self, username: &str, email: &str, password: &str) -> reqwest::Response {
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
                "owner={}&repo={}&path={}&git_ref=",
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

    pub async fn fetch_commit_diff(&self, owner: &str, repo: &str, sha: &str) -> reqwest::Response {
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

    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        description: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CREATE_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&title={}&description={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(title),
                urlencoded(description)
            ))
            .send()
            .await
            .expect("create_issue failed")
    }

    pub async fn list_issues(&self, owner: &str, repo: &str, status: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_ISSUES))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&status={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(status)
            ))
            .send()
            .await
            .expect("list_issues failed")
    }

    pub async fn get_issue(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}",
                urlencoded(owner),
                urlencoded(repo),
                number
            ))
            .send()
            .await
            .expect("get_issue failed")
    }

    pub async fn close_issue(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CLOSE_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}",
                urlencoded(owner),
                urlencoded(repo),
                number
            ))
            .send()
            .await
            .expect("close_issue failed")
    }

    pub async fn reopen_issue(&self, owner: &str, repo: &str, number: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_REOPEN_ISSUE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}",
                urlencoded(owner),
                urlencoded(repo),
                number
            ))
            .send()
            .await
            .expect("reopen_issue failed")
    }

    pub async fn add_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_ADD_COMMENT))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&number={}&body={}",
                urlencoded(owner),
                urlencoded(repo),
                number,
                urlencoded(body)
            ))
            .send()
            .await
            .expect("add_comment failed")
    }

    pub async fn explore_repos(&self, query: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_EXPLORE_REPOS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("query={}&remixable_only=false", urlencoded(query)))
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

    pub async fn add_collaborator(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
    ) -> reqwest::Response {
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

    pub async fn remove_collaborator_by_name(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
    ) -> reqwest::Response {
        // First list to find user_id, then remove
        let resp = self.list_collaborators(owner, repo).await;
        let body = resp.text().await.unwrap();
        // Extract user_id for the given username from the response
        // Simple approach: find the user_id near the username in the JSON
        let user_id = body
            .split("user_id")
            .skip(1)
            .find_map(|chunk| {
                if chunk.contains(username) || body.split(username).nth(1).is_some() {
                    chunk
                        .split(|c: char| !c.is_ascii_digit())
                        .find(|s| !s.is_empty())
                        .and_then(|s| s.parse::<i64>().ok())
                } else {
                    None
                }
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

    pub async fn install_ai_hook(
        &self,
        owner: &str,
        repo: &str,
        tool_id: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_INSTALL_AI_HOOK))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&tool_id={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(tool_id)
            ))
            .send()
            .await
            .expect("install_ai_hook request failed")
    }

    pub async fn check_installed_hooks(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CHECK_INSTALLED_HOOKS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("check_installed_hooks request failed")
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

    pub async fn get_diff_review(&self, owner: &str, repo: &str, sha: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_DIFF_REVIEW))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&sha={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(sha)
            ))
            .send()
            .await
            .expect("get_diff_review request failed")
    }

    pub async fn generate_diff_summary(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GENERATE_DIFF_SUMMARY))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&sha={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(sha)
            ))
            .send()
            .await
            .expect("generate_diff_summary request failed")
    }

    pub async fn fetch_remix_guide(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_REMIX_GUIDE))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("fetch_remix_guide request failed")
    }

    pub async fn list_repo_webhooks(&self, owner: &str, repo: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_REPO_WEBHOOKS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}",
                urlencoded(owner),
                urlencoded(repo)
            ))
            .send()
            .await
            .expect("list_repo_webhooks request failed")
    }

    pub async fn add_webhook(
        &self,
        owner: &str,
        repo: &str,
        url: &str,
        secret: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_ADD_WEBHOOK))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&url={}&secret={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(url),
                urlencoded(secret)
            ))
            .send()
            .await
            .expect("add_webhook request failed")
    }

    pub async fn fetch_pricing_info(&self) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_FETCH_PRICING_INFO))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("")
            .send()
            .await
            .expect("fetch_pricing_info request failed")
    }

    pub async fn delete_webhook(
        &self,
        owner: &str,
        repo: &str,
        webhook_id: i64,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_DELETE_WEBHOOK))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&webhook_id={}",
                urlencoded(owner),
                urlencoded(repo),
                webhook_id
            ))
            .send()
            .await
            .expect("delete_webhook request failed")
    }

    // --- Organization (Team) helpers ---

    pub async fn create_org(&self, slug: &str, display_name: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_CREATE_ORG))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "slug={}&display_name={}",
                urlencoded(slug),
                urlencoded(display_name)
            ))
            .send()
            .await
            .expect("create_org request failed")
    }

    pub async fn get_org_settings(&self, slug: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_GET_ORG_SETTINGS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("slug={}", urlencoded(slug)))
            .send()
            .await
            .expect("get_org_settings request failed")
    }

    pub async fn add_org_member(
        &self,
        slug: &str,
        username: &str,
        role: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_ADD_MEMBER))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "slug={}&username={}&role={}",
                urlencoded(slug),
                urlencoded(username),
                urlencoded(role)
            ))
            .send()
            .await
            .expect("add_org_member request failed")
    }

    pub async fn remove_org_member(&self, slug: &str, user_id: i64) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_REMOVE_MEMBER))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("slug={}&user_id={}", urlencoded(slug), user_id))
            .send()
            .await
            .expect("remove_org_member request failed")
    }

    pub async fn list_my_orgs(&self) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_LIST_MY_ORGS))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("")
            .send()
            .await
            .expect("list_my_orgs request failed")
    }

    pub async fn switch_org(&self, slug: &str) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_SWITCH_ORG))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("slug={}", urlencoded(slug)))
            .send()
            .await
            .expect("switch_org request failed")
    }

    pub async fn revert_session(
        &self,
        owner: &str,
        repo: &str,
        session_id: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, API_REVERT_SESSION))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "owner={}&repo={}&session_id={}",
                urlencoded(owner),
                urlencoded(repo),
                urlencoded(session_id)
            ))
            .send()
            .await
            .expect("revert_session request failed")
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
pub fn git_clone_ssh(port: u16, owner: &str, repo: &str, dest: &Path, key_path: &Path) -> Output {
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
pub fn create_commit_with_oxigit_context(
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

    // Write .oxigit/context.json
    let oxigit_dir = repo_dir.join(".oxigit");
    std::fs::create_dir_all(&oxigit_dir).expect("failed to create .oxigit dir");
    let mut ctx = serde_json::json!({ "tool": ai_tool });
    if let Some(model) = ai_model {
        ctx["model"] = serde_json::json!(model);
    }
    if let Some(prompt) = ai_prompt {
        ctx["prompt"] = serde_json::json!(prompt);
    }
    if let Some(session) = ai_session {
        ctx["session_id"] = serde_json::json!(session);
    }
    std::fs::write(oxigit_dir.join("context.json"), ctx.to_string())
        .expect("failed to write context.json");

    let status = Command::new("git")
        .args(["add", filename, ".oxigit/context.json"])
        .current_dir(repo_dir)
        .output()
        .expect("git add failed");
    assert!(status.status.success(), "git add failed");

    let status = Command::new("git")
        .args(["commit", "-m", subject])
        .current_dir(repo_dir)
        .env("GIT_AUTHOR_NAME", "Test User")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test User")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git commit failed");
    assert!(
        status.status.success(),
        "git commit with oxigit context failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

/// Create a file, add, and commit with AI trailers in the commit message (new approach).
/// Supports prompt_index for prompt-level grouping.
pub fn create_commit_with_trailers(
    repo_dir: &Path,
    filename: &str,
    content: &str,
    subject: &str,
    ai_tool: &str,
    ai_model: Option<&str>,
    ai_prompt: Option<&str>,
    ai_session: Option<&str>,
    ai_prompt_index: Option<i64>,
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
    let mut message = format!("{}\n\nOxigit-Tool: {}", subject, ai_tool);
    if let Some(model) = ai_model {
        message.push_str(&format!("\nOxigit-Model: {}", model));
    }
    if let Some(session) = ai_session {
        message.push_str(&format!("\nOxigit-Session: {}", session));
    }
    if let Some(prompt) = ai_prompt {
        message.push_str(&format!("\nOxigit-Prompt: {}", prompt));
    }
    if let Some(idx) = ai_prompt_index {
        message.push_str(&format!("\nOxigit-Prompt-Index: {}", idx));
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

/// Create a git branch from current HEAD.
pub fn create_branch(repo_dir: &Path, branch_name: &str) {
    let status = Command::new("git")
        .args(["branch", branch_name])
        .current_dir(repo_dir)
        .output()
        .expect("git branch failed");
    assert!(status.status.success(), "git branch failed");
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

/// Strip Leptos SSR hydration markers (`<!>`) so substring assertions work on
/// dynamic text that spans multiple text nodes.
pub fn strip_hydration_markers(html: &str) -> String {
    html.replace("<!>", "")
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
