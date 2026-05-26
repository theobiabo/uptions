mod app;
mod auth;
mod config;
pub mod db;
mod entities;
mod error;
mod polymarket;
pub mod users;

use crate::{app::create_app, app::state::AppState, config::AppConfig};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let config = AppConfig::from_env();
    let address = config.server_address.clone();
    let state = AppState::new(config).await?;
    let app = create_app(state);

    let listener = TcpListener::bind(&address)
        .await
        .expect("failed to bind listener");

    println!("Application is running on {}", address);

    axum::serve(listener, app).await.expect("failed to serve");

    Ok(())
}
