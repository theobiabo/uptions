mod app;
mod auth;
mod config;
pub mod db;
mod entities;
mod error;
pub mod libs;
mod polymarket;
mod response;
pub mod users;

use crate::{app::create_app, app::state::AppState, config::AppConfig};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_env();
    init_tracing();

    let config = AppConfig::from_env();
    let address = config.server_address.clone();
    let state = AppState::new(config).await?;
    let app = create_app(state);

    let listener = TcpListener::bind(&address)
        .await
        .expect("failed to bind listener");

    info!("Application is running on {}", address);

    axum::serve(listener, app).await.expect("failed to serve");

    Ok(())
}

fn load_env() {
    let manifest_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");

    if manifest_env.exists() {
        if let Err(error) = dotenvy::from_path(manifest_env) {
            panic!("failed to load .env: {error}");
        }
    } else {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(error) if error.not_found() => {}
            Err(error) => {
                panic!("failed to load .env: {error}");
            }
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("uptions_backend=debug,tower_http=info,axum=info"));

    fmt().with_env_filter(filter).compact().init();
}
