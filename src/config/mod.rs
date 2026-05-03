use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub server_address: String,
    pub polymarket_gamma_host: String,
    pub polymarket_clob_host: String,
    pub polymarket_chain_id: u64,
    pub polymarket_private_key: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            server_address: env::var("SERVER_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_owned()),
            polymarket_gamma_host: env::var("POLYMARKET_GAMMA_HOST")
                .unwrap_or_else(|_| "https://gamma-api.polymarket.com".to_owned()),
            polymarket_clob_host: env::var("POLYMARKET_CLOB_HOST")
                .unwrap_or_else(|_| "https://clob.polymarket.com".to_owned()),
            polymarket_chain_id: env::var("POLYMARKET_CHAIN_ID")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(137),
            polymarket_private_key: env::var("POLYMARKET_PRIVATE_KEY").ok(),
        }
    }
}
