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
