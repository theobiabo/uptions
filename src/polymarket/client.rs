use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use serde_json::Value;

use crate::{
    config::AppConfig,
    error::AppError,
    polymarket::dto::{MarketsQuery, OrderBookLevelResponse, OrderBookResponse},
};

#[derive(Clone)]
pub struct PolymarketClient {
    clob_host: String,
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
            clob_host: config.polymarket_clob_host.clone(),
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

    pub async fn fetch_order_book(&self, token_id: &str) -> Result<OrderBookResponse, AppError> {
        let token_id = token_id.trim();

        if token_id.is_empty() {
            return Err(AppError::BadRequest("token id is required".to_owned()));
        }

        let response = self
            .http_client
            .get(format!("{}/book", self.clob_host))
            .query(&[("token_id", token_id)])
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

        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;

        Ok(normalize_order_book(token_id, payload))
    }

    pub async fn fetch_market(&self, market_id: &str) -> Result<Value, AppError> {
        let query = MarketsQuery {
            id: Some(market_id.to_owned()),
            ..Default::default()
        };
        let markets = self.fetch_markets(&query).await?;

        match markets {
            Value::Array(items) => items
                .into_iter()
                .next()
                .ok_or_else(|| AppError::NotFound("Market not found".to_owned())),
            market if market.is_object() => Ok(market),
            _ => Err(AppError::ExternalApiError(
                "Unexpected Polymarket market payload".to_owned(),
            )),
        }
    }
}

fn normalize_order_book(token_id: &str, payload: Value) -> OrderBookResponse {
    let mut bids = levels_from_payload(&payload, "bids");
    let mut asks = levels_from_payload(&payload, "asks");

    bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    asks.sort_by(|a, b| b.price.total_cmp(&a.price));

    let max_usd = bids
        .iter()
        .chain(asks.iter())
        .map(|level| level.usd)
        .fold(0.0, f64::max);

    if max_usd > 0.0 {
        for level in bids.iter_mut().chain(asks.iter_mut()) {
            level.depth_percent = ((level.usd / max_usd) * 100.0).clamp(0.0, 100.0);
        }
    }

    let best_bid = bids.first().map(|level| level.price);
    let best_ask = asks.first().map(|level| level.price);
    let spread = best_bid
        .zip(best_ask)
        .map(|(bid, ask)| (ask - bid).max(0.0));

    OrderBookResponse {
        asks,
        best_ask,
        best_bid,
        bids,
        last_traded: number_from_keys(
            &payload,
            &["last_traded", "lastTradePrice", "last_trade_price"],
        ),
        spread,
        token_id: token_id.to_owned(),
        updated_at: Utc::now().to_rfc3339(),
    }
}

fn levels_from_payload(payload: &Value, key: &str) -> Vec<OrderBookLevelResponse> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(level_from_value).collect())
        .unwrap_or_default()
}

fn level_from_value(value: &Value) -> Option<OrderBookLevelResponse> {
    let price = number_from_keys(value, &["price", "p"])?;
    let shares = number_from_keys(value, &["size", "shares", "s"])?;

    if !price.is_finite() || !shares.is_finite() || price <= 0.0 || shares <= 0.0 {
        return None;
    }

    Some(OrderBookLevelResponse {
        depth_percent: 0.0,
        price,
        shares,
        usd: price * shares,
    })
}

fn number_from_keys(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| number_value(value.get(*key)?))
}

fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}
