use crate::{config::AppConfig, polymarket::client::PolymarketClient};

#[derive(Clone)]
pub struct AppState {
    pub polymarket_client: PolymarketClient,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            polymarket_client: PolymarketClient::new(&config),
        }
    }
}
