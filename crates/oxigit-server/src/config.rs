use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "oxigit", about = "Self-hosted Git platform")]
pub struct Config {
    /// Directory for data (SQLite DB, bare repos)
    #[arg(long, env = "OXIGIT_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    /// HTTP listen address
    #[arg(long, env = "OXIGIT_HTTP_ADDR", default_value = "127.0.0.1:3000")]
    pub http_addr: String,

    /// SSH listen address
    #[arg(long, env = "OXIGIT_SSH_ADDR", default_value = "127.0.0.1:2222")]
    pub ssh_addr: String,
}
