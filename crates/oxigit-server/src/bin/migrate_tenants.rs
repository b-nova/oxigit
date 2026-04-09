//! One-time migration script: splits a legacy single `oxigit.db` into
//! a control-plane `control.db` and per-user tenant databases.
//!
//! Usage:
//!   migrate-tenants --data-dir ./data

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    oxigit_core::migrate::migrate_legacy_to_multi_tenant(&args.data_dir).await?;

    println!("\nRestart the server to use the new layout.");

    Ok(())
}
