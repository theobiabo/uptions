use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde_json::Value;

use crate::{
    app::state::AppState,
    error::{AppError, ErrorResponse},
    polymarket::dto::{MarketsQuery, OrderBookResponse, VenueChainResponse},
    response::{ApiResponse, ok},
    venue::SupportedVenue,
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

#[utoipa::path(
    get,
    path = "/api/v1/polymarket/order-books/{token_id}",
    tag = "Polymarket",
    params(
        ("token_id" = String, Path, description = "Polymarket CLOB token id")
    ),
    responses(
        (status = 200, description = "Polymarket normalized order book", body = ApiResponse<OrderBookResponse>),
        (status = 400, description = "Invalid token id", body = ErrorResponse),
        (status = 502, description = "Upstream Polymarket error", body = ErrorResponse)
    )
)]
pub async fn fetch_order_book(
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<Json<ApiResponse<OrderBookResponse>>, AppError> {
    let order_book = state.polymarket_client.fetch_order_book(&token_id).await?;

    Ok(ok("Order book fetched successfully", order_book))
}

#[utoipa::path(
    get,
    path = "/api/v1/polymarket/venue-chain",
    tag = "Polymarket",
    responses(
        (status = 200, description = "Polymarket venue and chain configuration", body = ApiResponse<VenueChainResponse>)
    )
)]
pub async fn fetch_venue_chain() -> Json<ApiResponse<VenueChainResponse>> {
    let venue = SupportedVenue::Polymarket;
    let chain = venue.chain();

    ok(
        "Polymarket venue chain fetched successfully",
        VenueChainResponse {
            chain,
            chain_id: chain.chain_id(),
            chain_label: chain.label().to_owned(),
            venue,
            venue_id: venue.id().to_owned(),
            venue_label: venue.label().to_owned(),
        },
    )
}
