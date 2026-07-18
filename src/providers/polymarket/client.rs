use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE},
};

use hmac::{Hmac, Mac};
use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};
use sha2::Sha256;

use crate::{
    config::AppConfig,
    error::AppError,
    markets::types::MarketListQuery,
    providers::polymarket::{
        credentials::PolymarketApiCredentials,
        dto::{
            PolymarketMarket, PolymarketMarketsQuery, PolymarketSignedOrderPayload,
            PolymarketTokenMetadataResponse,
        },
    },
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, thiserror::Error)]
pub enum PolymarketSubmissionError {
    #[error("{0}")]
    Definite(String),
    #[error("{0}")]
    Ambiguous(String),
}

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

    pub async fn fetch_markets(
        &self,
        query: &MarketListQuery,
        offset: u32,
    ) -> Result<Vec<PolymarketMarket>, AppError> {
        let upstream_query = PolymarketMarketsQuery::new(query, offset);
        let response = self
            .http_client
            .get(format!("{}/markets", self.gamma_host))
            .query(&upstream_query)
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
            .json::<Vec<PolymarketMarket>>()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))
    }

    pub async fn fetch_order_book(&self, token_id: &str) -> Result<Value, AppError> {
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

        response
            .json::<Value>()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))
    }

    pub async fn fetch_market(&self, market_id: &str) -> Result<PolymarketMarket, AppError> {
        let query = MarketListQuery {
            id: Some(market_id.to_owned()),
            ..Default::default()
        };
        let markets = self.fetch_markets(&query, 0).await?;

        markets
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("Market not found".to_owned()))
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
    ) -> Result<Value, PolymarketSubmissionError> {
        let endpoint = "/order";
        let body = polymarket_order_payload(credentials, payload)
            .map_err(|error| PolymarketSubmissionError::Definite(error.to_string()))?;
        let body_text = serde_json::to_string(&body)
            .map_err(|error| PolymarketSubmissionError::Definite(error.to_string()))?;
        let timestamp = unix_timestamp().to_string();
        let signature = polymarket_hmac_signature(
            &credentials.secret,
            &timestamp,
            "POST",
            endpoint,
            Some(&body_text),
        )
        .map_err(|error| PolymarketSubmissionError::Definite(error.to_string()))?;
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
            .map_err(|error| PolymarketSubmissionError::Ambiguous(error.to_string()))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read upstream response".to_owned());
            return Err(classify_submission_http_error(status, body));
        }

        response
            .json::<Value>()
            .await
            .map_err(|error| PolymarketSubmissionError::Ambiguous(error.to_string()))
    }

    pub async fn get_order(
        &self,
        credentials: &PolymarketApiCredentials,
        order_id: &str,
    ) -> Result<Value, AppError> {
        let order_id = clean_order_id(order_id)?;
        let endpoint = format!("/data/order/{order_id}");
        self.send_l2_json(credentials, Method::GET, &endpoint, None, &[])
            .await
    }

    pub async fn get_trades(
        &self,
        credentials: &PolymarketApiCredentials,
        trade_id: &str,
    ) -> Result<Value, AppError> {
        let trade_id = trade_id.trim();
        if trade_id.is_empty() {
            return Err(AppError::BadRequest(
                "provider trade id is required".to_owned(),
            ));
        }
        let query = [
            ("id", trade_id),
            ("maker_address", credentials.funder.as_str()),
        ];
        self.send_l2_json(credentials, Method::GET, "/data/trades", None, &query)
            .await
    }

    pub async fn cancel_order(
        &self,
        credentials: &PolymarketApiCredentials,
        order_id: &str,
    ) -> Result<Value, AppError> {
        let order_id = clean_order_id(order_id)?;
        self.send_l2_json(
            credentials,
            Method::DELETE,
            "/order",
            Some(json!({"orderID": order_id})),
            &[],
        )
        .await
    }

    pub async fn cancel_orders(
        &self,
        credentials: &PolymarketApiCredentials,
        order_ids: &[String],
    ) -> Result<Value, AppError> {
        if order_ids.is_empty() || order_ids.len() > 1000 {
            return Err(AppError::BadRequest(
                "order_ids must contain between 1 and 1000 items".to_owned(),
            ));
        }
        for order_id in order_ids {
            clean_order_id(order_id)?;
        }
        self.send_l2_json(
            credentials,
            Method::DELETE,
            "/orders",
            Some(json!(order_ids)),
            &[],
        )
        .await
    }

    pub async fn cancel_all_orders(
        &self,
        credentials: &PolymarketApiCredentials,
    ) -> Result<Value, AppError> {
        self.send_l2_json(credentials, Method::DELETE, "/cancel-all", None, &[])
            .await
    }

    pub async fn cancel_market_orders(
        &self,
        credentials: &PolymarketApiCredentials,
        market: &str,
        asset_id: &str,
    ) -> Result<Value, AppError> {
        let market = market.trim();
        let asset_id = asset_id.trim();
        if market.is_empty() || asset_id.is_empty() {
            return Err(AppError::BadRequest(
                "market_id and token_id are required".to_owned(),
            ));
        }
        self.send_l2_json(
            credentials,
            Method::DELETE,
            "/cancel-market-orders",
            Some(json!({"market": market, "asset_id": asset_id})),
            &[],
        )
        .await
    }

    async fn send_l2_json(
        &self,
        credentials: &PolymarketApiCredentials,
        method: Method,
        endpoint: &str,
        body: Option<Value>,
        query: &[(&str, &str)],
    ) -> Result<Value, AppError> {
        let body_text = body
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let timestamp = unix_timestamp().to_string();
        let signature = polymarket_hmac_signature(
            &credentials.secret,
            &timestamp,
            method.as_str(),
            endpoint,
            body_text.as_deref(),
        )?;
        let mut request = self
            .http_client
            .request(method, format!("{}{}", self.clob_host, endpoint))
            .header("POLY_ADDRESS", credentials.address.as_str())
            .header("POLY_SIGNATURE", signature)
            .header("POLY_TIMESTAMP", timestamp)
            .header("POLY_API_KEY", credentials.api_key.as_str())
            .header("POLY_PASSPHRASE", credentials.passphrase.as_str());
        if !query.is_empty() {
            request = request.query(query);
        }
        if let Some(body_text) = body_text {
            request = request
                .header("Content-Type", "application/json")
                .body(body_text);
        }
        let response = request
            .send()
            .await
            .map_err(|error| AppError::ExternalApiError(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read upstream response".to_owned());
            return Err(AppError::ExternalApiError(format!(
                "Polymarket returned {status}: {body}"
            )));
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

fn classify_submission_http_error(
    status: StatusCode,
    message: String,
) -> PolymarketSubmissionError {
    if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
        PolymarketSubmissionError::Ambiguous(message)
    } else {
        PolymarketSubmissionError::Definite(message)
    }
}

fn clean_order_id(order_id: &str) -> Result<String, AppError> {
    let order_id = order_id.trim();
    if order_id.is_empty()
        || !order_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::BadRequest(
            "provider order id is invalid".to_owned(),
        ));
    }
    Ok(order_id.to_owned())
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

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{PolymarketSubmissionError, classify_submission_http_error};

    #[test]
    fn classifies_uncertain_http_submission_outcomes_for_reconciliation() {
        assert!(matches!(
            classify_submission_http_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "upstream".to_owned()
            ),
            PolymarketSubmissionError::Ambiguous(_)
        ));
        assert!(matches!(
            classify_submission_http_error(StatusCode::REQUEST_TIMEOUT, "timeout".to_owned()),
            PolymarketSubmissionError::Ambiguous(_)
        ));
        assert!(matches!(
            classify_submission_http_error(StatusCode::BAD_REQUEST, "rejected".to_owned()),
            PolymarketSubmissionError::Definite(_)
        ));
    }
}
