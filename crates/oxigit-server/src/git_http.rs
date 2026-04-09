use axum::{
    body::Body,
    extract::Path,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::process::Command;
use tracing;

use sqlx::SqlitePool;

use oxigit_core::{db, guardrail};
use oxigit_core::git::repo_path;

use crate::AppState;

/// Resolve the repo pool and filesystem path for an owner/repo pair.
/// In multi-tenant mode, looks up org_slug via repository_index, returns
/// the tenant pool and tenant-specific repo path.
/// In legacy mode, returns the control pool and standard repo path.
async fn resolve_repo(
    state: &AppState,
    owner: &str,
    repo_name: &str,
) -> Result<(SqlitePool, std::path::PathBuf), (StatusCode, &'static str)> {
    let control_pool = state.pool();
    if state.multi_tenant {
        let org_slug = db::lookup_repo_org(&control_pool, owner, repo_name)
            .await
            .map_err(|_| (StatusCode::NOT_FOUND, "Repository not found"))?;
        let pool = state
            .tenant_mgr
            .get_tenant_pool(&org_slug)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tenant"))?;
        let path = state
            .tenant_mgr
            .tenant_repos_dir(&org_slug)
            .join(format!("{}/{}.git", owner, repo_name));
        Ok((pool, path))
    } else {
        let path = repo_path(&state.data_dir, owner, repo_name);
        Ok((control_pool, path))
    }
}

/// Extract HTTP Basic Auth credentials from headers.
fn extract_basic_auth(headers: &HeaderMap) -> Option<(String, String)> {
    use base64::Engine;
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = auth.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(
        base64::engine::general_purpose::STANDARD.decode(encoded).ok()?
    ).ok()?;
    let mut parts = decoded.splitn(2, ':');
    let username = parts.next()?.to_string();
    let password = parts.next()?.to_string();
    Some((username, password))
}

/// WWW-Authenticate challenge response
fn auth_required() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, "Basic realm=\"Oxigit\"")
        .body(Body::from("Authentication required"))
        .unwrap()
}

/// GET /:owner/:repo.git/info/refs?service=git-upload-pack|git-receive-pack
pub async fn info_refs(
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let repo_name = repo.strip_suffix(".git").unwrap_or(&repo);
    let service = match params.get("service") {
        Some(s) if s == "git-upload-pack" || s == "git-receive-pack" => s.clone(),
        _ => {
            return (StatusCode::BAD_REQUEST, "Invalid service").into_response();
        }
    };

    // Resolve repo pool and path
    let control_pool = state.pool();
    let (repo_pool, path) = match resolve_repo(&state, &owner, repo_name).await {
        Ok(r) => r,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    // Verify repo exists and check access (users in control DB, repos in tenant DB)
    let (_, repo_db) = match db::get_repository_cross(&control_pool, &repo_pool, &owner, repo_name).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::NOT_FOUND, "Repository not found").into_response(),
    };

    // For private repos, require auth for all operations (including clone)
    if repo_db.is_private || service == "git-receive-pack" {
        match extract_basic_auth(&headers) {
            Some((username, password)) => {
                let auth_user = match db::authenticate_user(&control_pool, &username, &password).await {
                    Ok(u) => u,
                    Err(_) => return auth_required(),
                };
                // For push: check owner or collaborator
                if service == "git-receive-pack" {
                    if let Ok(can) = db::can_push_repo(&repo_pool, &repo_db, auth_user.id).await {
                        if !can {
                            return (StatusCode::FORBIDDEN, "Access denied").into_response();
                        }
                    }
                } else if repo_db.is_private && !db::can_access_repo(&repo_db, Some(auth_user.id)) {
                    return (StatusCode::FORBIDDEN, "Access denied").into_response();
                }
            }
            None => return auth_required(),
        }
    }
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found on disk").into_response();
    }

    // Run git to advertise refs
    // Strip "git-" prefix: "git-upload-pack" -> "upload-pack" as git subcommand
    let subcmd = service.strip_prefix("git-").unwrap_or(&service);
    let output = Command::new("git")
        .arg(subcmd)
        .arg("--stateless-rpc")
        .arg("--advertise-refs")
        .arg(&path)
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            tracing::error!("git {} failed: {}", service, String::from_utf8_lossy(&o.stderr));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
        Err(e) => {
            tracing::error!("Failed to spawn git: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to run git").into_response();
        }
    };

    // Build pkt-line header
    let header_line = format!("# service={}\n", service);
    let pkt_header = format!("{:04x}{}", header_line.len() + 4, header_line);

    let mut body = Vec::new();
    body.extend_from_slice(pkt_header.as_bytes());
    body.extend_from_slice(b"0000"); // flush-pkt
    body.extend_from_slice(&output.stdout);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            format!("application/x-{}-advertisement", service),
        )
        .header("Cache-Control", "no-cache")
        .body(Body::from(body))
        .unwrap()
}

/// POST /:owner/:repo.git/git-upload-pack
pub async fn upload_pack(
    Path((owner, repo)): Path<(String, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    body: axum::body::Bytes,
) -> Response {
    let repo_name = repo.strip_suffix(".git").unwrap_or(&repo);
    let (_, path) = match resolve_repo(&state, &owner, repo_name).await {
        Ok(r) => r,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    if !path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    run_git_service("git-upload-pack", &path, &body).await
}

/// POST /:owner/:repo.git/git-receive-pack
pub async fn receive_pack(
    Path((owner, repo)): Path<(String, String)>,
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<AppState>,
    body: axum::body::Bytes,
) -> Response {
    let repo_name = repo.strip_suffix(".git").unwrap_or(&repo);
    let control_pool = state.pool();
    let (repo_pool, path) = match resolve_repo(&state, &owner, repo_name).await {
        Ok(r) => r,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    // Require auth for push — owner or collaborator
    let repo_db_id;
    let repo_owner_id;
    match extract_basic_auth(&headers) {
        Some((username, password)) => {
            let auth_user = match db::authenticate_user(&control_pool, &username, &password).await {
                Ok(u) => u,
                Err(_) => return auth_required(),
            };
            let (_, repo_db) = match db::get_repository_cross(&control_pool, &repo_pool, &owner, repo_name).await {
                Ok(r) => r,
                Err(_) => return (StatusCode::NOT_FOUND, "Repository not found").into_response(),
            };
            repo_db_id = repo_db.id;
            repo_owner_id = repo_db.owner_id;
            if let Ok(can) = db::can_push_repo(&repo_pool, &repo_db, auth_user.id).await {
                if !can {
                    return (StatusCode::FORBIDDEN, "Access denied").into_response();
                }
            }
        }
        None => return auth_required(),
    }
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    // Capture refs before push for AI metadata processing
    let before_refs = oxigit_core::git::capture_refs(&path).unwrap_or_default();

    // Pass environment for pre-receive hook (guardrail blocking)
    let http_addr = state.leptos_options.site_addr.to_string();
    let port = http_addr.rsplit(':').next().unwrap_or("9100").to_string();
    let secret_hex = hex::encode(&state.secret_key);
    let env_vars = vec![
        ("OXIGIT_PORT".to_string(), port),
        ("OXIGIT_SECRET".to_string(), secret_hex),
        ("REPO_ID".to_string(), repo_db_id.to_string()),
    ];
    let response = run_git_service_with_env("git-receive-pack", &path, &body, &env_vars).await;

    // After push: capture refs again and process AI trailers + webhooks in background
    let path_clone = path.clone();
    let owner_clone = owner.clone();
    let repo_name_clone = repo_name.to_string();
    let base_url = format!("http://{}", state.leptos_options.site_addr);
    tokio::spawn(async move {
        let after_refs = oxigit_core::git::capture_refs(&path_clone).unwrap_or_default();
        oxigit_core::hooks::process_post_receive(
            &repo_pool,
            &control_pool,
            &path_clone,
            repo_db_id,
            repo_owner_id,
            &before_refs,
            &after_refs,
            &owner_clone,
            &repo_name_clone,
            &base_url,
        )
        .await;
    });

    response
}

/// POST /api/deploy-callback/:commit_sha — Receive deploy preview URL from external service.
pub async fn deploy_callback(
    Path(commit_sha): Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(payload): axum::Json<DeployCallbackPayload>,
) -> Response {
    let control_pool = state.pool();
    let (repo_pool, _) = match resolve_repo(&state, &payload.repo_owner, &payload.repo_name).await {
        Ok(r) => r,
        Err((status, msg)) => return (status, msg).into_response(),
    };

    let (_, repo_db) = match db::get_repository_cross(&control_pool, &repo_pool, &payload.repo_owner, &payload.repo_name).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::NOT_FOUND, "Repository not found").into_response(),
    };

    // Gate deploy previews to Pro+ plans
    let owner_plan = db::get_user_plan(&control_pool, repo_db.owner_id)
        .await
        .unwrap_or_else(|_| "free".into());
    let ent = oxigit_core::entitlements::for_plan(&owner_plan);
    if !ent.deploy_previews {
        return (StatusCode::FORBIDDEN, "Deploy previews require a Pro or higher plan").into_response();
    }

    match db::update_deploy_preview(
        &repo_pool,
        repo_db.id,
        &commit_sha,
        &payload.preview_url,
        &payload.status,
    )
    .await
    {
        Ok(_) => (StatusCode::OK, "OK").into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Deploy preview not found").into_response(),
    }
}

#[derive(::serde::Deserialize)]
pub struct DeployCallbackPayload {
    pub repo_owner: String,
    pub repo_name: String,
    pub status: String,
    pub preview_url: String,
}

async fn run_git_service(service: &str, repo_path: &std::path::Path, input: &[u8]) -> Response {
    run_git_service_with_env(service, repo_path, input, &[]).await
}

async fn run_git_service_with_env(service: &str, repo_path: &std::path::Path, input: &[u8], env_vars: &[(String, String)]) -> Response {
    use tokio::io::AsyncWriteExt;

    // Strip "git-" prefix: "git-upload-pack" -> "upload-pack" as git subcommand
    let subcmd = service.strip_prefix("git-").unwrap_or(service);
    let mut cmd = Command::new("git");
    cmd.arg(subcmd)
        .arg("--stateless-rpc")
        .arg(repo_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, val) in env_vars {
        cmd.env(key, val);
    }
    let mut child = match cmd.spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to spawn git {}: {}", service, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to run git").into_response();
        }
    };

    // Write request body to stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(input).await {
            tracing::error!("Failed to write to git stdin: {}", e);
        }
        drop(stdin);
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("git {} failed: {}", service, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
    };

    if !output.status.success() {
        tracing::error!(
            "git {} exited with {}: {}",
            service,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            format!("application/x-{}-result", service),
        )
        .header("Cache-Control", "no-cache")
        .body(Body::from(output.stdout))
        .unwrap()
}

/// POST /internal/guardrail-check — Called by pre-receive hook to validate push.
pub async fn guardrail_check(
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<AppState>,
    body: String,
) -> Response {
    // Validate internal secret
    let expected_secret = hex::encode(&state.secret_key);
    let provided_secret = headers
        .get("X-Internal-Secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided_secret != expected_secret {
        return (StatusCode::FORBIDDEN, "Invalid secret").into_response();
    }

    // Parse form-urlencoded body
    let params: std::collections::HashMap<String, String> = body
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((percent_decode(k), percent_decode(v)))
        })
        .collect();

    let repo_id: i64 = match params.get("repo_id").and_then(|v| v.parse().ok()) {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "Missing repo_id").into_response(),
    };

    let diff = params.get("diff").cloned().unwrap_or_default();
    let file_count: usize = params.get("file_count").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let commit_ref = params.get("ref").cloned().unwrap_or_default();
    let new_sha = params.get("new").cloned().unwrap_or_default();

    // Guardrail check uses repo_id directly — we need to find which tenant pool has it.
    // The repo_id comes from the pre-receive hook env, so in multi-tenant mode we need
    // to look up by repo_id. For now, use the control pool in legacy mode or search
    // by the repo_owner/repo_name params if available. Since the pre-receive hook only
    // passes repo_id, we use the control pool for guardrail queries in legacy mode,
    // and in multi-tenant mode we'd need the repo_owner/repo_name from the hook.
    // TODO: Pass owner/repo through pre-receive hook env for multi-tenant guardrail lookup.
    let pool = state.pool();

    // Load block rules only
    let rules = match db::get_guardrail_rules(&pool, repo_id).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::OK, "OK").into_response(),
    };
    let block_rules: Vec<_> = rules.into_iter().filter(|r| r.action == "block").collect();

    if block_rules.is_empty() {
        return (StatusCode::OK, "OK").into_response();
    }

    let config = db::get_guardrail_config(&pool, repo_id).await.ok().flatten();

    let violations = guardrail::evaluate_diff(&block_rules, &config, &diff, file_count);
    let blocking = violations.iter().filter(|v| v.action == "block").collect::<Vec<_>>();

    if blocking.is_empty() {
        return (StatusCode::OK, "OK").into_response();
    }

    // Log violations
    for v in &blocking {
        let _ = db::insert_guardrail_violation(
            &pool, repo_id, &new_sha, Some(&commit_ref),
            &v.category, "blocked", &v.severity, &v.message,
            v.file_path.as_deref(), None,
        ).await;
    }

    let message = guardrail::format_block_message(&violations);
    (StatusCode::OK, format!("BLOCKED\n{}", message)).into_response()
}

/// Simple percent-decoding for form data.
fn percent_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(c);
        }
    }
    result
}
