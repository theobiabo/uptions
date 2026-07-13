use std::net::SocketAddr;

use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};
use uptions_backend::{app::create_app, app::state::AppState, config::AppConfig, load_env};

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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("failed to serve");

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("uptions_backend=debug,tower_http=info,axum=info"));

    fmt().with_env_filter(filter).compact().init();
}
