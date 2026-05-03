use std::time::Duration;

use reqwest::Client;
use serde_json::Value;

use crate::{config::AppConfig, error::AppError, polymarket::dto::MarketsQuery};

#[derive(Clone)]
pub struct PolymarketClient {
    http_client: Client,
    gamma_host: String,
}

impl PolymarketClient {
    pub fn new(config: &AppConfig) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("polymarket http client should build");

        Self {
            http_client,
            gamma_host: config.polymarket_gamma_host.clone(),
        }
    }

    pub async fn fetch_markets(&self, query: &MarketsQuery) -> Result<Value, AppError> {
        let response = self
            .http_client
            .get(format!("{}/markets", self.gamma_host))
            .query(query)
            .send()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read upstream response".to_owned());
            return Err(AppError::ExternalApiError(body));
        }

        response
            .json::<Value>()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))
    }
}
