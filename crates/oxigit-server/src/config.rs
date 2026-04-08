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

    /// LLM provider for AI diff summaries (none/openai/anthropic/ollama).
    /// Defaults to "ollama" so AI features work out of the box with a local Ollama instance.
    #[arg(long, env = "OXIGIT_LLM_PROVIDER", default_value = "ollama")]
    pub llm_provider: String,

    /// API key for cloud LLM providers
    #[arg(long, env = "OXIGIT_LLM_API_KEY")]
    pub llm_api_key: Option<String>,

    /// LLM model name (e.g., "qwen3-coder", "gpt-4o-mini", "claude-haiku-4-5-20251001")
    #[arg(long, env = "OXIGIT_LLM_MODEL", default_value = "qwen3-coder")]
    pub llm_model: String,

    /// Base URL for Ollama or custom LLM endpoints
    #[arg(long, env = "OXIGIT_LLM_BASE_URL")]
    pub llm_base_url: Option<String>,

    /// Stripe secret key (sk_test_... or sk_live_...)
    #[arg(long, env = "STRIPE_SECRET_KEY")]
    pub stripe_secret_key: Option<String>,

    /// Stripe webhook signing secret (whsec_...)
    #[arg(long, env = "STRIPE_WEBHOOK_SECRET")]
    pub stripe_webhook_secret: Option<String>,

    /// Stripe publishable key (pk_test_... or pk_live_...)
    #[arg(long, env = "STRIPE_PUBLISHABLE_KEY")]
    pub stripe_publishable_key: Option<String>,

    /// Stripe Price ID for Flat plan
    #[arg(long, env = "STRIPE_PRICE_FLAT")]
    pub stripe_price_flat: Option<String>,

    /// Stripe Price ID for Team plan
    #[arg(long, env = "STRIPE_PRICE_TEAM")]
    pub stripe_price_team: Option<String>,

    /// Stripe Price ID for Founding Member plan
    #[arg(long, env = "STRIPE_PRICE_FOUNDING")]
    pub stripe_price_founding: Option<String>,

    /// SMTP host for sending notification emails
    #[arg(long, env = "OXIGIT_SMTP_HOST")]
    pub smtp_host: Option<String>,

    /// SMTP port (default 587)
    #[arg(long, env = "OXIGIT_SMTP_PORT", default_value = "587")]
    pub smtp_port: u16,

    /// SMTP username
    #[arg(long, env = "OXIGIT_SMTP_USER")]
    pub smtp_user: Option<String>,

    /// SMTP password
    #[arg(long, env = "OXIGIT_SMTP_PASSWORD")]
    pub smtp_password: Option<String>,

    /// From address for outgoing emails
    #[arg(long, env = "OXIGIT_SMTP_FROM")]
    pub smtp_from: Option<String>,

    /// Email address to receive contact form inquiries
    #[arg(long, env = "OXIGIT_CONTACT_EMAIL")]
    pub contact_email: Option<String>,
}
