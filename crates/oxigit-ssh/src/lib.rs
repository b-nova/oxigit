use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use russh::keys::{Algorithm, HashAlg, PrivateKey, PublicKey};
use russh::server::{Auth, Handler, Msg, Session};
use russh::{Channel, ChannelId, ChannelMsg};
use sqlx::SqlitePool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use oxigit_core::db;
use oxigit_core::git::repo_path;

/// Start the SSH server on the given address.
pub async fn run_ssh_server(
    addr: SocketAddr,
    pool: SqlitePool,
    data_dir: PathBuf,
    host_key: PrivateKey,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    };
    let config = Arc::new(config);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("SSH server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        tracing::debug!("SSH connection from {}", peer_addr);

        let handler = OxigitSshHandler {
            pool: pool.clone(),
            data_dir: data_dir.clone(),
            authenticated_user: None,
            channels: HashMap::new(),
        };

        let config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = russh::server::run_stream(config, stream, handler).await {
                tracing::debug!("SSH session from {} ended: {}", peer_addr, e);
            }
        });
    }
}

/// Generate or load an SSH host key from the data directory.
pub fn load_or_generate_host_key(data_dir: &std::path::Path) -> PrivateKey {
    let key_path = data_dir.join("ssh_host_ed25519_key");

    if key_path.exists() {
        let key_data = std::fs::read_to_string(&key_path).expect("Failed to read host key");
        PrivateKey::from_openssh(&key_data).expect("Failed to parse host key")
    } else {
        tracing::info!("Generating new SSH host key at {}", key_path.display());
        let key = PrivateKey::random(&mut rand_core::OsRng, Algorithm::Ed25519)
            .expect("Failed to generate host key");
        let key_str = key
            .to_openssh(russh_keys::ssh_key::LineEnding::LF)
            .expect("Failed to serialize host key");
        // key_str is Zeroizing<String>, write the bytes
        std::fs::write(&key_path, key_str.as_bytes()).expect("Failed to write host key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .expect("Failed to set host key permissions");
        }
        key
    }
}

struct OxigitSshHandler {
    pool: SqlitePool,
    data_dir: PathBuf,
    authenticated_user: Option<String>,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

impl Handler for OxigitSshHandler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        let fingerprint = public_key.fingerprint(HashAlg::Sha256).to_string();
        tracing::debug!("SSH auth attempt with fingerprint: {}", fingerprint);

        match db::find_user_by_ssh_fingerprint(&self.pool, &fingerprint).await {
            Ok(user) => {
                tracing::info!("SSH authenticated user: {}", user.username);
                self.authenticated_user = Some(user.username);
                Ok(Auth::Accept)
            }
            Err(_) => {
                tracing::debug!("SSH key not found: {}", fingerprint);
                Ok(Auth::Reject {
                    proceed_with_methods: None,
                })
            }
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.channels.insert(channel.id(), channel);
        Ok(true)
    }

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).to_string();
        tracing::debug!("SSH exec: {}", command);

        let username = match &self.authenticated_user {
            Some(u) => u.clone(),
            None => {
                let _ = session.channel_failure(channel_id);
                return Ok(());
            }
        };

        let (service, repo_ref) = match parse_git_command(&command) {
            Some(parsed) => parsed,
            None => {
                tracing::warn!("Invalid SSH command: {}", command);
                let _ = session.channel_failure(channel_id);
                return Ok(());
            }
        };

        let (owner, repo_name) = match parse_repo_path(&repo_ref) {
            Some(parsed) => parsed,
            None => {
                tracing::warn!("Invalid repo path: {}", repo_ref);
                let _ = session.channel_failure(channel_id);
                return Ok(());
            }
        };

        // For push, verify the user has write access (owner or collaborator)
        let mut repo_db_id: Option<i64> = None;
        if service == "git-receive-pack" {
            match db::get_repository(&self.pool, &owner, &repo_name).await {
                Ok((_, repo_db)) => {
                    let push_user = db::get_user_by_username(&self.pool, &username).await;
                    let can_push = match push_user {
                        Ok(u) => db::can_push_repo(&self.pool, &repo_db, u.id).await.unwrap_or(false),
                        Err(_) => false,
                    };
                    if !can_push {
                        tracing::warn!("SSH push denied for {} on {}/{}", username, owner, repo_name);
                        let _ = session.channel_failure(channel_id);
                        return Ok(());
                    }
                    repo_db_id = Some(repo_db.id);
                }
                Err(_) => {
                    let _ = session.channel_failure(channel_id);
                    return Ok(());
                }
            }
        }

        let path = repo_path(&self.data_dir, &owner, &repo_name);
        if !path.exists() {
            let _ = session.channel_failure(channel_id);
            return Ok(());
        }

        let _ = session.channel_success(channel_id);

        // Take channel ownership and spawn git process
        let mut channel = match self.channels.remove(&channel_id) {
            Some(ch) => ch,
            None => return Ok(()),
        };

        let pool = self.pool.clone();
        let is_receive = service == "git-receive-pack";
        let ssh_owner = owner.clone();
        let ssh_repo = repo_name.clone();
        tokio::spawn(async move {
            if let Err(e) = run_git_over_channel(&service, &path, &mut channel, is_receive, repo_db_id, &pool, &ssh_owner, &ssh_repo).await {
                tracing::error!("Git SSH error: {}", e);
            }
        });

        Ok(())
    }
}

async fn run_git_over_channel(
    service: &str,
    repo_path: &std::path::Path,
    channel: &mut Channel<Msg>,
    is_receive: bool,
    repo_db_id: Option<i64>,
    pool: &SqlitePool,
    owner: &str,
    repo_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Capture refs before push for AI metadata processing
    let before_refs = if is_receive {
        oxigit_core::git::capture_refs(repo_path).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Strip "git-" prefix: "git-upload-pack" -> "upload-pack" as git subcommand
    let subcmd = service.strip_prefix("git-").unwrap_or(service);
    let mut child = Command::new("git")
        .arg(subcmd)
        .arg(repo_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut child_stdin = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();

    // Use channel.make_writer() for sending data to the channel
    // and channel.wait() for receiving data from the channel.
    // But we need to do both concurrently, so we'll use the lower-level approach:
    // spawn tasks for stdout/stderr writing via the channel directly.

    // We need to:
    // 1. Forward channel data -> git stdin
    // 2. Forward git stdout -> channel
    // 3. Forward git stderr -> channel extended data
    // Since Channel isn't Clone, we use make_reader/make_writer for the I/O.

    // Actually, the simplest approach for the server side:
    // Use channel.wait() in a loop to get incoming data, forward to stdin.
    // Meanwhile, read stdout and send via channel.data().

    // git stdout -> channel (using a separate task with a pipe)
    let (stdout_tx, mut stdout_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let stdout_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 32768];
        loop {
            match child_stdout.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if stdout_tx.send(buf[..n].to_vec()).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // git stderr -> collect
    let mut child_stderr = child.stderr.take().unwrap();
    let stderr_task = tokio::spawn(async move {
        let mut stderr_buf = Vec::new();
        let _ = child_stderr.read_to_end(&mut stderr_buf).await;
        stderr_buf
    });

    // Main loop: multiplex channel input and git stdout
    loop {
        tokio::select! {
            // Data from git stdout -> send to channel
            Some(data) = stdout_rx.recv() => {
                channel.data(&data[..]).await?;
            }
            // Data from SSH client -> send to git stdin
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        child_stdin.write_all(data).await?;
                    }
                    Some(ChannelMsg::Eof) => {
                        drop(child_stdin);
                        // Wait for git to finish
                        let status = child.wait().await?;
                        let exit_code = status.code().unwrap_or(1) as u32;

                        // Process AI metadata after successful push
                        if is_receive && exit_code == 0 {
                            if let Some(rid) = repo_db_id {
                                let after_refs = oxigit_core::git::capture_refs(repo_path).unwrap_or_default();
                                oxigit_core::hooks::process_post_receive(
                                    pool,
                                    repo_path,
                                    rid,
                                    &before_refs,
                                    &after_refs,
                                    owner,
                                    repo_name,
                                    "",
                                )
                                .await;
                            }
                        }

                        // Drain remaining stdout
                        while let Ok(data) = stdout_rx.try_recv() {
                            let _ = channel.data(&data[..]).await;
                        }
                        let _ = stdout_task.await;

                        // Send stderr
                        if let Ok(stderr_data) = stderr_task.await {
                            if !stderr_data.is_empty() {
                                let _ = channel.extended_data(1, &stderr_data[..]).await;
                            }
                        }

                        channel.exit_status(exit_code).await?;
                        channel.eof().await?;
                        channel.close().await?;
                        return Ok(());
                    }
                    Some(ChannelMsg::Close) | None => {
                        // Client disconnected
                        let _ = child.kill().await;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn parse_git_command(cmd: &str) -> Option<(String, String)> {
    let cmd = cmd.trim();
    let (service, rest) = if let Some(rest) = cmd.strip_prefix("git-upload-pack ") {
        ("git-upload-pack".to_string(), rest)
    } else if let Some(rest) = cmd.strip_prefix("git-receive-pack ") {
        ("git-receive-pack".to_string(), rest)
    } else if let Some(rest) = cmd.strip_prefix("git upload-pack ") {
        ("git-upload-pack".to_string(), rest)
    } else if let Some(rest) = cmd.strip_prefix("git receive-pack ") {
        ("git-receive-pack".to_string(), rest)
    } else {
        return None;
    };

    let path = rest.trim().trim_matches('\'').trim_matches('"').to_string();
    Some((service, path))
}

fn parse_repo_path(path: &str) -> Option<(String, String)> {
    use oxigit_core::auth::{validate_repo_name, validate_username};

    let path = path.trim_start_matches('/');
    let parts: Vec<&str> = path.splitn(2, '/').collect();
    if parts.len() != 2 {
        return None;
    }
    let owner = parts[0].to_string();
    let repo = parts[1]
        .strip_suffix(".git")
        .unwrap_or(parts[1])
        .to_string();

    // Validate to prevent path traversal
    if validate_username(&owner).is_err() || validate_repo_name(&repo).is_err() {
        return None;
    }

    Some((owner, repo))
}
