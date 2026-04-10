#[cfg(feature = "saas")]
use std::sync::Arc;

use axum::{
    Extension, Router,
    routing::{get, post},
};
use clap::Parser;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

use oxigit_app::server_fns::AppState;
use oxigit_app::{App, Shell, ShellProps};
use oxigit_core::db;
#[cfg(feature = "saas")]
use oxigit_core::tenant::TenantPoolManager;

#[cfg(feature = "saas")]
mod badge;
mod config;
mod git_http;
#[cfg(feature = "saas")]
mod stripe_webhook;

use config::Config;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::parse();

    // Ensure data directories exist
    std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");

    // Database initialization
    #[cfg(feature = "saas")]
    let (pool, multi_tenant, tenant_mgr) = {
        let control_db_path = config.data_dir.join("control.db");
        let legacy_db_path = config.data_dir.join("oxigit.db");

        let multi_tenant = if config.legacy_mode {
            false
        } else if control_db_path.exists() {
            true
        } else if legacy_db_path.exists() {
            tracing::info!("Detected legacy oxigit.db — auto-migrating to multi-tenant...");
            oxigit_core::migrate::migrate_legacy_to_multi_tenant(&config.data_dir)
                .await
                .expect("Auto-migration from legacy DB failed");
            true
        } else {
            true
        };

        let pool = if multi_tenant {
            let url = format!("sqlite:{}?mode=rwc", control_db_path.display());
            let p = db::create_pool(&url)
                .await
                .expect("Failed to create control database pool");
            db::run_control_migrations(&p)
                .await
                .expect("Failed to run control migrations");
            tracing::info!(
                "Multi-tenant mode: control DB at {}",
                control_db_path.display()
            );
            p
        } else {
            std::fs::create_dir_all(config.data_dir.join("repos"))
                .expect("Failed to create repos directory");
            let url = format!("sqlite:{}?mode=rwc", legacy_db_path.display());
            let p = db::create_pool(&url)
                .await
                .expect("Failed to create database pool");
            db::run_migrations(&p)
                .await
                .expect("Failed to run migrations");
            tracing::info!("Legacy mode: database at {}", legacy_db_path.display());
            p
        };

        let tenant_mgr = Arc::new(TenantPoolManager::new(
            pool.clone(),
            config.data_dir.clone(),
            50,
        ));
        (pool, multi_tenant, tenant_mgr)
    };

    #[cfg(not(feature = "saas"))]
    let pool = {
        std::fs::create_dir_all(config.data_dir.join("repos"))
            .expect("Failed to create repos directory");
        let db_path = config.data_dir.join("oxigit.db");
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let p = db::create_pool(&url)
            .await
            .expect("Failed to create database pool");
        db::run_migrations(&p)
            .await
            .expect("Failed to run migrations");
        tracing::info!("Database at {}", db_path.display());
        p
    };

    // SSH host key
    let host_key = oxigit_ssh::load_or_generate_host_key(&config.data_dir);

    // Leptos configuration
    let leptos_options = LeptosOptions::builder()
        .output_name("oxigit")
        .site_root("target/site")
        .site_pkg_dir("pkg")
        .site_addr(
            config
                .http_addr
                .parse::<std::net::SocketAddr>()
                .expect("Invalid HTTP address"),
        )
        .reload_port(9101)
        .build();

    let routes = generate_route_list(App);

    // Load or generate secret key for session signing
    let secret_key = load_or_generate_secret(&config);

    let state = AppState {
        #[cfg(feature = "saas")]
        tenant_mgr: tenant_mgr.clone(),
        #[cfg(feature = "saas")]
        multi_tenant,
        pool: pool.clone(),
        data_dir: config.data_dir.clone(),
        secret_key,
        leptos_options: leptos_options.clone(),
        llm_provider: config.llm_provider.clone(),
        llm_api_key: config.llm_api_key.clone(),
        llm_model: config.llm_model.clone(),
        llm_base_url: config.llm_base_url.clone(),
        #[cfg(feature = "saas")]
        stripe_secret_key: config.stripe_secret_key.clone(),
        #[cfg(feature = "saas")]
        stripe_publishable_key: config.stripe_publishable_key.clone(),
        #[cfg(feature = "saas")]
        stripe_webhook_secret: config.stripe_webhook_secret.clone(),
        #[cfg(feature = "saas")]
        stripe_price_flat: config.stripe_price_flat.clone(),
        #[cfg(feature = "saas")]
        stripe_price_team: config.stripe_price_team.clone(),
        #[cfg(feature = "saas")]
        stripe_price_founding: config.stripe_price_founding.clone(),
        smtp_host: config.smtp_host.clone(),
        smtp_port: config.smtp_port,
        smtp_user: config.smtp_user.clone(),
        smtp_password: config.smtp_password.clone(),
        smtp_from: config.smtp_from.clone(),
        contact_email: config.contact_email.clone(),
    };

    // Git Smart HTTP routes + deploy callback (must be before Leptos routes)
    let mut git_routes = Router::new()
        .route("/{owner}/{repo}/info/refs", get(git_http::info_refs))
        .route(
            "/{owner}/{repo}/git-upload-pack",
            post(git_http::upload_pack),
        )
        .route(
            "/{owner}/{repo}/git-receive-pack",
            post(git_http::receive_pack),
        )
        .route(
            "/api/deploy-callback/{commit_sha}",
            post(git_http::deploy_callback),
        )
        .route("/internal/guardrail-check", post(git_http::guardrail_check));

    #[cfg(feature = "saas")]
    {
        git_routes = git_routes
            .route("/api/stripe/webhook", post(stripe_webhook::handle_webhook))
            .route("/api/badge/{username_svg}", get(badge::founding_badge));
    }

    let git_routes = git_routes.with_state(state.clone());

    // Build router — use Shell for SSR, App for hydration
    let shell_options = leptos_options.clone();
    let app = git_routes
        .merge(
            Router::new()
                .leptos_routes(&leptos_options, routes, {
                    let opts = shell_options.clone();
                    move || {
                        Shell(ShellProps {
                            options: opts.clone(),
                        })
                    }
                })
                .fallback(leptos_axum::file_and_error_handler(
                    move |opts: LeptosOptions| {
                        move || {
                            Shell(ShellProps {
                                options: opts.clone(),
                            })
                        }
                    },
                ))
                .with_state(leptos_options),
        )
        .nest_service("/pkg", ServeDir::new("target/site/pkg"))
        .nest_service("/brand", ServeDir::new("target/site/brand"))
        .route_service(
            "/favicon.svg",
            tower_http::services::ServeFile::new("target/site/favicon.svg"),
        )
        .route_service(
            "/favicon-16x16.png",
            tower_http::services::ServeFile::new("target/site/favicon-16x16.png"),
        )
        .route_service(
            "/favicon-32x32.png",
            tower_http::services::ServeFile::new("target/site/favicon-32x32.png"),
        )
        .route_service(
            "/apple-touch-icon.png",
            tower_http::services::ServeFile::new("target/site/apple-touch-icon.png"),
        )
        .route_service(
            "/site.webmanifest",
            tower_http::services::ServeFile::new("target/site/site.webmanifest"),
        )
        .route_service(
            "/og-image.png",
            tower_http::services::ServeFile::new("target/site/og-image.png"),
        )
        .layer(Extension(state));

    // Start SSH server in background
    let ssh_addr: std::net::SocketAddr = config.ssh_addr.parse().expect("Invalid SSH address");
    let ssh_data_dir = config.data_dir.clone();
    #[cfg(feature = "saas")]
    {
        let ssh_tenant_mgr = tenant_mgr.clone();
        tokio::spawn(async move {
            if let Err(e) = oxigit_ssh::run_ssh_server(
                ssh_addr,
                ssh_tenant_mgr,
                ssh_data_dir,
                host_key,
                multi_tenant,
            )
            .await
            {
                tracing::error!("SSH server error: {}", e);
            }
        });
    }
    #[cfg(not(feature = "saas"))]
    {
        let ssh_pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) =
                oxigit_ssh::run_ssh_server(ssh_addr, ssh_pool, ssh_data_dir, host_key).await
            {
                tracing::error!("SSH server error: {}", e);
            }
        });
    }

    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(&config.http_addr)
        .await
        .expect("Failed to bind HTTP listener");

    tracing::info!("Oxigit listening on http://{}", config.http_addr);
    tracing::info!("SSH server on ssh://{}", config.ssh_addr);

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}

fn load_or_generate_secret(config: &Config) -> Vec<u8> {
    // If provided via config/env, use it
    if let Some(ref key_hex) = config.secret_key {
        return hex::decode(key_hex).expect("OXIGIT_SECRET_KEY must be valid hex");
    }

    // Otherwise, load from file or generate
    let key_path = config.data_dir.join("secret_key");
    if key_path.exists() {
        let key_hex = std::fs::read_to_string(&key_path).expect("Failed to read secret_key file");
        hex::decode(key_hex.trim()).expect("Invalid hex in secret_key file")
    } else {
        use rand_core::RngCore;
        let mut key = vec![0u8; 32];
        rand_core::OsRng.fill_bytes(&mut key);
        let key_hex = hex::encode(&key);
        std::fs::write(&key_path, &key_hex).expect("Failed to write secret_key file");
        tracing::info!("Generated new secret key at {}", key_path.display());
        key
    }
}
