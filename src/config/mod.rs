use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub server_address: String,
    pub database_url: String,
    pub polymarket_gamma_host: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            server_address: env::var("SERVER_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_owned()),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            polymarket_gamma_host: env::var("POLYMARKET_GAMMA_HOST")
                .unwrap_or_else(|_| "https://gamma-api.polymarket.com".to_owned()),
        }
    }
}
