use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::providers::{
    polymarket::dto::{PolymarketExecutionType, PolymarketTokenMetadataResponse},
    types::ProviderId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeSide {
    Buy,
    Sell,
}

impl TradeSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeOrderType {
    Market,
    Limit,
}

impl TradeOrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Market => "MARKET",
            Self::Limit => "LIMIT",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeIntentStatus {
    PendingSignature,
    Submitting,
    ReconciliationRequired,
    Submitted,
    Matched,
    Mined,
    Retrying,
    Filled,
    PartiallyFilled,
    Rejected,
    Failed,
    CancellationRequested,
    Cancelled,
}

impl TradeIntentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingSignature => "pending_signature",
            Self::Submitting => "submitting",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Submitted => "submitted",
            Self::Matched => "matched",
            Self::Mined => "mined",
            Self::Retrying => "retrying",
            Self::Filled => "filled",
            Self::PartiallyFilled => "partially_filled",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::CancellationRequested => "cancellation_requested",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTradeIntentRequest {
    pub amount: f64,
    pub automation_id: Option<String>,
    #[serde(default)]
    pub defer_exec: bool,
    #[schema(example = "540818")]
    pub market_id: String,
    #[schema(example = "World Cup: Golden Boot Winner")]
    pub market_title: String,
    #[schema(example = "YES")]
    pub outcome: String,
    pub price: Option<f64>,
    pub provider: ProviderId,
    pub side: TradeSide,
    pub order_type: TradeOrderType,
    pub execution_type: PolymarketExecutionType,
    #[serde(default)]
    pub post_only: bool,
    #[schema(example = "123456789")]
    pub token_id: String,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitSignedTradeRequest {
    #[serde(default)]
    pub defer_exec: bool,
    pub execution_type: PolymarketExecutionType,
    pub post_only: Option<bool>,
    #[schema(value_type = Object)]
    pub signed_order: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TradeIntentResponse {
    pub amount: f64,
    pub automation_id: Option<String>,
    pub chain: String,
    pub chain_id: i64,
    pub created_at: String,
    pub defer_exec: bool,
    pub error: Option<String>,
    pub execution_type: String,
    pub id: String,
    pub market_id: String,
    pub market_title: String,
    pub order_type: String,
    pub outcome: String,
    pub post_only: bool,
    pub price: Option<f64>,
    pub provider: String,
    pub provider_order_id: Option<String>,
    #[schema(value_type = Object)]
    pub provider_response: Option<Value>,
    pub reconciliation_checked_at: Option<String>,
    pub side: String,
    pub signed_order_hash: Option<String>,
    pub signed_maker_amount_base: Option<String>,
    pub signed_taker_amount_base: Option<String>,
    pub normalized_amount_base: Option<String>,
    pub normalized_price_numerator: Option<String>,
    pub normalized_price_denominator: Option<String>,
    pub status: String,
    pub submission_started_at: Option<String>,
    pub submitted_at: Option<String>,
    pub cancellation_requested_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub token_id: String,
    pub updated_at: String,
    pub wallet_address: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateTradeIntentResponse {
    pub trade: TradeIntentResponse,
    pub token_metadata: PolymarketTokenMetadataResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitSignedTradeResponse {
    #[schema(value_type = Object)]
    pub provider_response: Value,
    pub trade: TradeIntentResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReconcileTradeResponse {
    pub provider_lookup_available: bool,
    pub resolution: String,
    pub trade: TradeIntentResponse,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelMultipleTradesRequest {
    #[schema(min_items = 1, max_items = 1000)]
    pub trade_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelMarketTradesRequest {
    pub provider: ProviderId,
    pub market_id: String,
    pub token_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CancelTradesResponse {
    #[schema(value_type = Object)]
    pub provider_response: Value,
    pub trades: Vec<TradeIntentResponse>,
}
