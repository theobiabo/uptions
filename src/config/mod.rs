use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub server_address: String,
    pub polymarket_gamma_host: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            server_address: env::var("SERVER_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_owned()),
            polymarket_gamma_host: env::var("POLYMARKET_GAMMA_HOST")
                .unwrap_or_else(|_| "https://gamma-api.polymarket.com".to_owned()),
        }
    }
}
