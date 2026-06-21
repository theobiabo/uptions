use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde_json::Value;

use crate::{
    app::state::AppState,
    error::{AppError, ErrorResponse},
    polymarket::dto::MarketsQuery,
    response::{ApiResponse, ok},
};

#[utoipa::path(
    get,
    path = "/api/v1/polymarket/markets",
    tag = "Polymarket",
    params(MarketsQuery),
    responses(
        (status = 200, description = "Raw Polymarket markets payload", body = ApiResponse<Value>),
        (status = 502, description = "Upstream Polymarket error", body = ErrorResponse)
    )
)]
pub async fn fetch_markets(
    State(state): State<AppState>,
    Query(query): Query<MarketsQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let markets = state.polymarket_client.fetch_markets(&query).await?;

    Ok(ok("Markets fetched successfully", markets))
}

#[utoipa::path(
    get,
    path = "/api/v1/polymarket/markets/{market_id}",
    tag = "Polymarket",
    params(
        ("market_id" = String, Path, description = "Polymarket market id")
    ),
    responses(
        (status = 200, description = "Raw Polymarket market payload", body = ApiResponse<Value>),
        (status = 404, description = "Market not found", body = ErrorResponse),
        (status = 502, description = "Upstream Polymarket error", body = ErrorResponse)
    )
)]
pub async fn fetch_market(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let market = state.polymarket_client.fetch_market(&market_id).await?;

    Ok(ok("Market fetched successfully", market))
}
