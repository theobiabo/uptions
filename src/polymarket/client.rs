use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::{Value, json};
use sha2::Sha256;

use crate::{
    config::AppConfig,
    error::AppError,
    polymarket::dto::{
        MarketsQuery, OrderBookLevelResponse, OrderBookResponse, PolymarketApiCredentials,
        PolymarketSignedOrderPayload, PolymarketTokenMetadataResponse,
    },
};

type HmacSha256 = Hmac<Sha256>;

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

    pub async fn fetch_token_metadata(
        &self,
        token_id: &str,
    ) -> Result<PolymarketTokenMetadataResponse, AppError> {
        let token_id = clean_token_id(token_id)?;
        let tick_size = self.fetch_tick_size(&token_id).await?;
        let negative_risk = self.fetch_negative_risk(&token_id).await?;
        let fee_rate_bps = self.fetch_fee_rate_bps(&token_id).await?;

        Ok(PolymarketTokenMetadataResponse {
            fee_rate_bps,
            negative_risk,
            tick_size,
            token_id,
        })
    }

    pub async fn submit_signed_order(
        &self,
        credentials: &PolymarketApiCredentials,
        payload: &PolymarketSignedOrderPayload,
    ) -> Result<Value, AppError> {
        let endpoint = "/order";
        let body = polymarket_order_payload(credentials, payload)?;
        let body_text = serde_json::to_string(&body)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let timestamp = unix_timestamp().to_string();
        let signature = polymarket_hmac_signature(
            &credentials.secret,
            &timestamp,
            "POST",
            endpoint,
            Some(&body_text),
        )?;
        let response = self
            .http_client
            .post(format!("{}{}", self.clob_host, endpoint))
            .header("Content-Type", "application/json")
            .header("POLY_ADDRESS", credentials.address.as_str())
            .header("POLY_SIGNATURE", signature)
            .header("POLY_TIMESTAMP", timestamp)
            .header("POLY_API_KEY", credentials.api_key.as_str())
            .header("POLY_PASSPHRASE", credentials.passphrase.as_str())
            .body(body_text)
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

    async fn fetch_tick_size(&self, token_id: &str) -> Result<String, AppError> {
        let payload = self
            .fetch_clob_json("/tick-size", &[("token_id", token_id)])
            .await?;

        payload
            .get("minimum_tick_size")
            .and_then(value_to_string)
            .ok_or_else(|| AppError::ExternalApiError("missing Polymarket tick size".to_owned()))
    }

    async fn fetch_negative_risk(&self, token_id: &str) -> Result<bool, AppError> {
        let payload = self
            .fetch_clob_json("/neg-risk", &[("token_id", token_id)])
            .await?;

        payload
            .get("neg_risk")
            .and_then(Value::as_bool)
            .ok_or_else(|| AppError::ExternalApiError("missing Polymarket neg risk".to_owned()))
    }

    async fn fetch_fee_rate_bps(&self, token_id: &str) -> Result<u64, AppError> {
        let payload = self
            .fetch_clob_json("/fee-rate", &[("token_id", token_id)])
            .await?;

        payload
            .get("base_fee")
            .and_then(value_to_u64)
            .ok_or_else(|| AppError::ExternalApiError("missing Polymarket fee rate".to_owned()))
    }

    async fn fetch_clob_json(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> Result<Value, AppError> {
        let response = self
            .http_client
            .get(format!("{}{}", self.clob_host, endpoint))
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

fn clean_token_id(token_id: &str) -> Result<String, AppError> {
    let token_id = token_id.trim();

    if token_id.is_empty() {
        return Err(AppError::BadRequest("token id is required".to_owned()));
    }

    Ok(token_id.to_owned())
}

fn polymarket_order_payload(
    credentials: &PolymarketApiCredentials,
    payload: &PolymarketSignedOrderPayload,
) -> Result<Value, AppError> {
    let order = &payload.signed_order;
    let side = match order.get("side") {
        Some(Value::Number(number)) if number.as_u64() == Some(0) => "BUY",
        Some(Value::Number(number)) if number.as_u64() == Some(1) => "SELL",
        Some(Value::String(value)) if value.eq_ignore_ascii_case("BUY") => "BUY",
        Some(Value::String(value)) if value.eq_ignore_ascii_case("SELL") => "SELL",
        _ => {
            return Err(AppError::BadRequest(
                "signed order side is invalid".to_owned(),
            ));
        }
    };
    let body = json!({
        "deferExec": payload.defer_exec,
        "order": {
            "salt": order_salt(order)?,
            "maker": required_order_field(order, "maker")?,
            "signer": required_order_field(order, "signer")?,
            "taker": required_order_field(order, "taker")?,
            "tokenId": required_order_field(order, "tokenId")?,
            "makerAmount": required_order_field(order, "makerAmount")?,
            "takerAmount": required_order_field(order, "takerAmount")?,
            "side": side,
            "expiration": required_order_field(order, "expiration")?,
            "nonce": required_order_field(order, "nonce")?,
            "feeRateBps": required_order_field(order, "feeRateBps")?,
            "signatureType": required_order_field(order, "signatureType")?,
            "signature": required_order_field(order, "signature")?
        },
        "owner": credentials.api_key,
        "orderType": payload.execution_type,
        "postOnly": payload.post_only.unwrap_or(false)
    });

    Ok(body)
}

fn required_order_field(order: &Value, key: &str) -> Result<Value, AppError> {
    order
        .get(key)
        .cloned()
        .ok_or_else(|| AppError::BadRequest(format!("signed order {key} is required")))
}

fn order_salt(order: &Value) -> Result<Value, AppError> {
    match required_order_field(order, "salt")? {
        Value::String(value) => value
            .parse::<u64>()
            .map(|value| json!(value))
            .map_err(|_| AppError::BadRequest("signed order salt is invalid".to_owned())),
        value => Ok(value),
    }
}

fn polymarket_hmac_signature(
    secret: &str,
    timestamp: &str,
    method: &str,
    request_path: &str,
    body: Option<&str>,
) -> Result<String, AppError> {
    let key = decode_polymarket_secret(secret)?;
    let mut mac = HmacSha256::new_from_slice(&key)
        .map_err(|_| AppError::BadRequest("Polymarket secret is invalid".to_owned()))?;
    let mut message = format!("{timestamp}{method}{request_path}");

    if let Some(body) = body {
        message.push_str(body);
    }

    mac.update(message.as_bytes());
    Ok(URL_SAFE.encode(mac.finalize().into_bytes()))
}

fn decode_polymarket_secret(secret: &str) -> Result<Vec<u8>, AppError> {
    let sanitized: String = secret
        .replace('-', "+")
        .replace('_', "/")
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '=')
        })
        .collect();

    STANDARD
        .decode(sanitized)
        .map_err(|_| AppError::BadRequest("Polymarket secret is invalid".to_owned()))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_secs()
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.to_owned()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_to_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(value) => value
            .as_u64()
            .or_else(|| value.as_f64().map(|value| value.round() as u64)),
        Value::String(value) => value.parse::<u64>().ok(),
        _ => None,
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
