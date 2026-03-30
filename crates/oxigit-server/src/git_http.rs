use axum::{
    body::Body,
    extract::Path,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::process::Command;
use tracing;

use oxigit_core::db;
use oxigit_core::git::repo_path;

use crate::AppState;

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

    // Verify repo exists and check access
    let pool = &state.pool;
    let (_, repo_db) = match db::get_repository(pool, &owner, repo_name).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::NOT_FOUND, "Repository not found").into_response(),
    };

    // For private repos, require auth for all operations (including clone)
    if repo_db.is_private || service == "git-receive-pack" {
        match extract_basic_auth(&headers) {
            Some((username, password)) => {
                let auth_user = match db::authenticate_user(pool, &username, &password).await {
                    Ok(u) => u,
                    Err(_) => return auth_required(),
                };
                // For push: check owner or collaborator
                if service == "git-receive-pack" {
                    if let Ok(can) = db::can_push_repo(pool, &repo_db, auth_user.id).await {
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

    let path = repo_path(&state.data_dir, &owner, repo_name);
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
    let path = repo_path(&state.data_dir, &owner, repo_name);

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
    let pool = &state.pool;

    // Require auth for push — owner or collaborator
    let repo_db_id;
    match extract_basic_auth(&headers) {
        Some((username, password)) => {
            let auth_user = match db::authenticate_user(pool, &username, &password).await {
                Ok(u) => u,
                Err(_) => return auth_required(),
            };
            let (_, repo_db) = match db::get_repository(pool, &owner, repo_name).await {
                Ok(r) => r,
                Err(_) => return (StatusCode::NOT_FOUND, "Repository not found").into_response(),
            };
            repo_db_id = repo_db.id;
            if let Ok(can) = db::can_push_repo(pool, &repo_db, auth_user.id).await {
                if !can {
                    return (StatusCode::FORBIDDEN, "Access denied").into_response();
                }
            }
        }
        None => return auth_required(),
    }

    let path = repo_path(&state.data_dir, &owner, repo_name);
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    // Capture refs before push for AI metadata processing
    let before_refs = oxigit_core::git::capture_refs(&path).unwrap_or_default();

    let response = run_git_service("git-receive-pack", &path, &body).await;

    // After push: capture refs again and process AI trailers + webhooks in background
    let pool_clone = pool.clone();
    let path_clone = path.clone();
    let owner_clone = owner.clone();
    let repo_name_clone = repo_name.to_string();
    let base_url = format!("http://{}", state.leptos_options.site_addr);
    tokio::spawn(async move {
        let after_refs = oxigit_core::git::capture_refs(&path_clone).unwrap_or_default();
        oxigit_core::hooks::process_post_receive(
            &pool_clone,
            &path_clone,
            repo_db_id,
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
    let pool = &state.pool;

    let (_, repo_db) = match db::get_repository(pool, &payload.repo_owner, &payload.repo_name).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::NOT_FOUND, "Repository not found").into_response(),
    };

    match db::update_deploy_preview(
        pool,
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
    use tokio::io::AsyncWriteExt;

    // Strip "git-" prefix: "git-upload-pack" -> "upload-pack" as git subcommand
    let subcmd = service.strip_prefix("git-").unwrap_or(service);
    let mut child = match Command::new("git")
        .arg(subcmd)
        .arg("--stateless-rpc")
        .arg(repo_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
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
