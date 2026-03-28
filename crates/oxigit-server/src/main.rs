use axum::Router;
use clap::Parser;
use leptos::prelude::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

use oxigit_app::App;
use oxigit_app::server_fns::AppState;
use oxigit_core::db;

mod config;

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
    std::fs::create_dir_all(config.data_dir.join("repos"))
        .expect("Failed to create repos directory");

    // Database setup
    let db_path = config.data_dir.join("oxigit.db");
    let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = db::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");
    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database initialized at {}", db_path.display());

    // Leptos configuration
    let leptos_options = LeptosOptions::builder()
        .output_name("oxigit")
        .site_root("target/site")
        .site_pkg_dir("pkg")
        .site_addr(config.http_addr.parse::<std::net::SocketAddr>().expect("Invalid HTTP address"))
        .reload_port(3001)
        .build();

    let routes = generate_route_list(App);

    let state = AppState {
        pool,
        data_dir: config.data_dir.clone(),
        leptos_options: leptos_options.clone(),
    };

    // Build router
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let state = state.clone();
            move || {
                provide_context(state.clone());
                view! { <App /> }
            }
        })
        .fallback(leptos_axum::file_and_error_handler(move |_opts| {
            let state = state.clone();
            move || {
                provide_context(state.clone());
                view! { <App /> }
            }
        }))
        .nest_service("/pkg", ServeDir::new("target/site/pkg"))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&config.http_addr)
        .await
        .expect("Failed to bind HTTP listener");

    tracing::info!("Oxigit listening on http://{}", config.http_addr);

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}
