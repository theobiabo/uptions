mod error;
mod services;
mod dtos;
pub mod utils;

use axum::{Router, routing::get};
use tokio::net::TcpListener;

const ADDRESS: &str = "0.0.0.0:3000";

async fn health_check() -> &'static str {
    "Uptions endpoint is running"
}

fn create_app() -> Router {
    Router::new().route("/", get(health_check))
}

#[tokio::main]
async fn main() {
    let app = create_app();

    let listener = TcpListener::bind(ADDRESS)
        .await
        .expect("failed to bind listener");

    println!("Application is running on {}", ADDRESS);

    axum::serve(listener, app).await.expect("failed to serve");
}
