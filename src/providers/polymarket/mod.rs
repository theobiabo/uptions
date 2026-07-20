pub(crate) mod client;
pub mod config;
pub mod connection;
pub mod credentials;
pub mod dto;

pub mod market_data;
pub mod user_stream;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;

use crate::{
    config::AppConfig,
    error::AppError,
    markets::types::{MarketListQuery, MarketPageResponse, OrderBookResponse},
    providers::{
        registry::{ResolvedInstrument, ResolvedMarket},
        types::ProviderId,
    },
};

use self::{
    client::{PolymarketClient, PolymarketSubmissionError},
    credentials::PolymarketApiCredentials,
    dto::{PolymarketSignedOrderPayload, PolymarketTokenMetadataResponse},
};

#[derive(Clone)]
pub struct PolymarketAdapter {
    client: PolymarketClient,
}

impl PolymarketAdapter {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            client: PolymarketClient::new(config),
        }
    }

    pub async fn fetch_markets(
        &self,
        query: &MarketListQuery,
    ) -> Result<MarketPageResponse, AppError> {
        if query.limit.is_some_and(|limit| !(1..=500).contains(&limit)) {
            return Err(AppError::BadRequest(
                "market limit must be between 1 and 500".to_owned(),
            ));
        }

        let offset = decode_market_cursor(query.cursor.as_deref())?
            .unwrap_or_else(|| query.offset.unwrap_or_default());
        let raw_markets = self.client.fetch_markets(query, offset).await?;
        let returned = u32::try_from(raw_markets.len()).unwrap_or(u32::MAX);
        let markets = raw_markets
            .into_iter()
            .map(market_data::normalize_market)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = query
            .limit
            .filter(|limit| returned == *limit)
            .map(|_| encode_market_cursor(offset.saturating_add(returned)));
        let chain = self.chain();

        Ok(MarketPageResponse {
            provider: ProviderId::Polymarket,
            chain,
            chain_id: chain.id(),
            markets,
            next_cursor,
        })
    }

    pub async fn fetch_order_book(
        &self,
        market_id: &str,
        outcome_id: &str,
    ) -> Result<OrderBookResponse, AppError> {
        let market = self.resolve_market(market_id).await?;
        let instrument = market_data::resolve_order_book_outcome(&market, outcome_id)?;
        let payload = self.client.fetch_order_book(&instrument.token_id).await?;
        Ok(market_data::normalize_order_book(
            &instrument.market_id,
            &instrument.outcome,
            &instrument.token_id,
            payload,
        ))
    }

    const fn chain(&self) -> crate::providers::types::Chain {
        crate::providers::types::Chain::Polygon
    }

    pub async fn resolve_market(&self, market_id: &str) -> Result<ResolvedMarket, AppError> {
        market_data::resolve_market(self.client.fetch_market(market_id).await?, market_id)
    }

    pub async fn resolve_instrument(
        &self,
        market_id: &str,
        token_id: &str,
        outcome: &str,
    ) -> Result<(ResolvedInstrument, PolymarketTokenMetadataResponse), AppError> {
        let market = self.resolve_market(market_id).await?;
        let instrument = market_data::resolve_instrument(&market, token_id, outcome)?;
        let metadata = self
            .client
            .fetch_token_metadata(&instrument.token_id)
            .await?;
        Ok((instrument, metadata))
    }

    pub async fn submit_signed_order(
        &self,
        credentials: &PolymarketApiCredentials,
        payload: &PolymarketSignedOrderPayload,
    ) -> Result<Value, PolymarketSubmissionError> {
        self.client.submit_signed_order(credentials, payload).await
    }

    pub async fn get_order(
        &self,
        credentials: &PolymarketApiCredentials,
        order_id: &str,
    ) -> Result<Value, AppError> {
        self.client.get_order(credentials, order_id).await
    }

    pub async fn get_trades(
        &self,
        credentials: &PolymarketApiCredentials,
        trade_id: &str,
    ) -> Result<Value, AppError> {
        self.client.get_trades(credentials, trade_id).await
    }

    pub async fn cancel_order(
        &self,
        credentials: &PolymarketApiCredentials,
        order_id: &str,
    ) -> Result<Value, AppError> {
        self.client.cancel_order(credentials, order_id).await
    }

    pub async fn cancel_orders(
        &self,
        credentials: &PolymarketApiCredentials,
        order_ids: &[String],
    ) -> Result<Value, AppError> {
        self.client.cancel_orders(credentials, order_ids).await
    }

    pub async fn cancel_all_orders(
        &self,
        credentials: &PolymarketApiCredentials,
    ) -> Result<Value, AppError> {
        self.client.cancel_all_orders(credentials).await
    }

    pub async fn cancel_market_orders(
        &self,
        credentials: &PolymarketApiCredentials,
        market_id: &str,
        token_id: &str,
    ) -> Result<Value, AppError> {
        self.client
            .cancel_market_orders(credentials, market_id, token_id)
            .await
    }
}

fn encode_market_cursor(offset: u32) -> String {
    URL_SAFE_NO_PAD.encode(offset.to_string())
}

fn decode_market_cursor(cursor: Option<&str>) -> Result<Option<u32>, AppError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor.trim())
        .ok()
        .and_then(|value| String::from_utf8(value).ok())
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| AppError::BadRequest("market cursor is invalid".to_owned()))?;
    Ok(Some(decoded))
}
