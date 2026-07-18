use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::venue::{SupportedChain, SupportedVenue};

#[derive(Debug, Deserialize, Serialize, Default, IntoParams, ToSchema)]
pub struct MarketsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub slug: Option<String>,
    pub id: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct VenueChainResponse {
    pub chain: SupportedChain,
    pub chain_id: u64,
    pub chain_label: String,
    pub venue: SupportedVenue,
    pub venue_id: String,
    pub venue_label: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct OrderBookLevelResponse {
    pub depth_percent: f64,
    pub price: f64,
    pub shares: f64,
    pub usd: f64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct OrderBookResponse {
    pub asks: Vec<OrderBookLevelResponse>,
    pub best_ask: Option<f64>,
    pub best_bid: Option<f64>,
    pub bids: Vec<OrderBookLevelResponse>,
    pub last_traded: Option<f64>,
    pub spread: Option<f64>,
    pub token_id: String,
    pub updated_at: String,
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

#[derive(Clone, Debug)]
pub struct PolymarketApiCredentials {
    pub address: String,
    pub funder: String,
    pub signature_type: i32,
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
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
