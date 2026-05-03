mod app;
mod auth;
mod config;
mod error;
mod polymarket;

use tokio::net::TcpListener;

use crate::{app::create_app, app::state::AppState, config::AppConfig};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = AppConfig::from_env();
    let address = config.server_address.clone();
    let state = AppState::new(config);
    let app = create_app(state);

    let listener = TcpListener::bind(&address)
        .await
        .expect("failed to bind listener");

    println!("Application is running on {}", address);

    axum::serve(listener, app).await.expect("failed to serve");
}
