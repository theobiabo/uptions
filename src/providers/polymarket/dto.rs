use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::markets::types::MarketListQuery;

#[derive(Debug, Serialize)]
pub(crate) struct PolymarketMarketsQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<&'a str>,
}

impl<'a> PolymarketMarketsQuery<'a> {
    pub fn new(query: &'a MarketListQuery, offset: u32) -> Self {
        Self {
            limit: query.limit,
            offset: Some(offset),
            active: query.active,
            closed: query.closed,
            archived: query.archived,
            slug: query.slug.as_deref(),
            id: query.id.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PolymarketMarket {
    #[serde(default)]
    pub id: Value,

    #[serde(default)]
    pub question: Value,
    #[serde(default)]
    pub title: Value,

    #[serde(default)]
    pub description: Value,
    #[serde(default)]
    pub category: Value,
    #[serde(default)]
    pub image: Value,
    #[serde(default)]
    pub url: Value,
    #[serde(default)]
    pub icon: Value,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    #[serde(default)]
    pub outcomes: Value,
    #[serde(default, alias = "clob_token_ids")]
    pub clob_token_ids: Value,
    #[serde(default)]
    pub outcome_prices: Value,
    pub accepting_orders: Option<bool>,

    #[serde(default)]
    pub order_min_size: Value,
    #[serde(default)]
    pub order_price_min_tick_size: Value,
    pub neg_risk: Option<bool>,
    #[serde(default)]
    pub best_bid: Value,
    #[serde(default)]
    pub best_ask: Value,
    #[serde(default)]
    pub last_trade_price: Value,
    #[serde(default)]
    pub one_day_price_change: Value,

    #[serde(default)]
    pub volume_num: Value,
    #[serde(default)]
    pub volume: Value,
    #[serde(default)]
    pub liquidity_num: Value,
    #[serde(default)]
    pub liquidity: Value,
    #[serde(default)]
    pub start_date: Value,
    #[serde(default)]
    pub end_date: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "event_type", rename_all = "lowercase")]
pub enum PolymarketUserEvent {
    Order(PolymarketOrderEvent),
    Trade(PolymarketTradeEvent),
}

impl PolymarketUserEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Order(_) => "order",
            Self::Trade(_) => "trade",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PolymarketOrderEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub update_type: String,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub market: Option<String>,
    pub asset_id: Option<String>,
    pub side: Option<String>,
    pub original_size: Option<String>,
    pub size_matched: Option<String>,
    pub price: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PolymarketTradeMakerOrder {
    pub order_id: String,
    pub owner: Option<String>,
    pub maker_address: Option<String>,
    pub matched_amount: Option<String>,
    pub price: Option<String>,
    pub asset_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PolymarketTradeEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub update_type: String,
    pub taker_order_id: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub market: Option<String>,
    pub asset_id: Option<String>,
    pub side: Option<String>,
    pub size: Option<String>,
    pub price: Option<String>,
    pub last_update: Option<String>,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub maker_orders: Vec<PolymarketTradeMakerOrder>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolymarketExecutionType {
    Fok,
    Fak,
    Gtc,
    Gtd,
}

impl PolymarketExecutionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fok => "FOK",
            Self::Fak => "FAK",
            Self::Gtc => "GTC",
            Self::Gtd => "GTD",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct PolymarketSignedOrderPayload {
    #[schema(value_type = Object)]
    pub signed_order: serde_json::Value,
    pub execution_type: String,
    #[serde(default)]
    pub defer_exec: bool,
    pub post_only: Option<bool>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct PolymarketTokenMetadataResponse {
    pub fee_rate_bps: u64,
    pub negative_risk: bool,
    pub tick_size: String,
    pub token_id: String,
}
