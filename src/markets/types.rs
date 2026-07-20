use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::providers::types::{Chain, ChainId, ProviderId};

#[derive(Clone, Debug, Default, Deserialize, Serialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct MarketListQuery {
    #[param(minimum = 1, maximum = 500)]
    pub limit: Option<u32>,
    #[param(minimum = 0)]
    pub offset: Option<u32>,
    pub cursor: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub slug: Option<String>,
    pub id: Option<String>,
}

/// Provider-neutral page returned after an adapter normalizes its upstream payload.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketPageResponse {
    pub provider: ProviderId,
    pub chain: Chain,
    pub chain_id: ChainId,
    pub markets: Vec<MarketResponse>,
    pub next_cursor: Option<String>,
}

/// Canonical market contract exposed by every provider route.
#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketResponse {
    pub id: String,
    pub provider: ProviderId,
    pub chain: Chain,
    pub chain_id: ChainId,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub image_url: Option<String>,
    pub external_url: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub accepting_orders: bool,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub volume: Option<f64>,
    pub liquidity: Option<f64>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub last_trade_price: Option<f64>,
    pub price_change_24h: Option<f64>,
    pub outcomes: Vec<MarketOutcomeResponse>,
    pub trading: MarketTradingMetadata,
}

impl MarketResponse {
    pub fn outcome(&self, outcome_id: &str, outcome: &str) -> Option<&MarketOutcomeResponse> {
        self.outcomes.iter().find(|candidate| {
            candidate.id.as_deref() == Some(outcome_id)
                && candidate.label.eq_ignore_ascii_case(outcome)
        })
    }

    pub fn outcome_by_id(&self, outcome_id: &str) -> Option<&MarketOutcomeResponse> {
        self.outcomes
            .iter()
            .find(|candidate| candidate.id.as_deref() == Some(outcome_id))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketOutcomeResponse {
    pub id: Option<String>,
    pub label: String,
    pub price: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketTradingMetadata {
    pub minimum_order_size: Option<f64>,
    pub minimum_tick_size: Option<f64>,
    pub negative_risk: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct OrderBookResponse {
    pub provider: ProviderId,
    pub chain: Chain,
    pub chain_id: ChainId,
    pub market_id: String,
    pub outcome_id: String,
    pub outcome: String,
    pub asks: Vec<OrderBookLevel>,
    pub best_ask: Option<f64>,
    pub best_bid: Option<f64>,
    pub bids: Vec<OrderBookLevel>,
    pub last_traded: Option<f64>,
    pub spread: Option<f64>,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct OrderBookLevel {
    pub depth_percent: f64,
    pub price: f64,
    pub shares: f64,
    pub usd: f64,
}
