use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "oxigit", about = "Self-hosted Git platform")]
pub struct Config {
    /// Directory for data (SQLite DB, bare repos)
    #[arg(long, env = "OXIGIT_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    /// HTTP listen address
    #[arg(long, env = "OXIGIT_HTTP_ADDR", default_value = "127.0.0.1:9100")]
    pub http_addr: String,

    /// SSH listen address
    #[arg(long, env = "OXIGIT_SSH_ADDR", default_value = "127.0.0.1:2222")]
    pub ssh_addr: String,

    /// Secret key for signing session cookies (hex-encoded, 32+ bytes recommended).
    /// Auto-generated and saved to data_dir/secret_key if not provided.
    #[arg(long, env = "OXIGIT_SECRET_KEY")]
    pub secret_key: Option<String>,
}
