use crate::{auth::service::AuthService, config::AppConfig, polymarket::client::PolymarketClient};

#[derive(Clone)]
pub struct AppState {
    pub auth_service: AuthService,
    pub polymarket_client: PolymarketClient,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            auth_service: AuthService::new(),
            polymarket_client: PolymarketClient::new(&config),
        }
    }
}
