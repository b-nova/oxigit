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

    /// LLM provider for AI diff summaries (none/openai/anthropic/ollama)
    #[arg(long, env = "OXIGIT_LLM_PROVIDER", default_value = "none")]
    pub llm_provider: String,

    /// API key for cloud LLM providers
    #[arg(long, env = "OXIGIT_LLM_API_KEY")]
    pub llm_api_key: Option<String>,

    /// LLM model name (e.g., "gpt-4o-mini", "claude-haiku-4-5-20251001")
    #[arg(long, env = "OXIGIT_LLM_MODEL", default_value = "gpt-4o-mini")]
    pub llm_model: String,

    /// Base URL for Ollama or custom LLM endpoints
    #[arg(long, env = "OXIGIT_LLM_BASE_URL")]
    pub llm_base_url: Option<String>,
}
