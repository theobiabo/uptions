use std::str::FromStr;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    app::state::AppState,
    error::{AppError, ErrorResponse},
    markets::types::{MarketListQuery, MarketPageResponse, MarketResponse, OrderBookResponse},
    providers::types::{ProviderId, ProviderResponse},
    response::{ApiResponse, ok},
};

#[utoipa::path(
    get,
    path = "/api/v1/providers",
    tag = "Providers",
    responses(
        (status = 200, description = "Available provider catalog", body = ApiResponse<Vec<ProviderResponse>>)
    )
)]
pub async fn list_providers(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ProviderResponse>>> {
    ok("Providers fetched successfully", state.providers.catalog())
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider}",
    tag = "Providers",
    params(("provider" = String, Path, description = "Provider route id")),
    responses(
        (status = 200, description = "Provider details and capabilities", body = ApiResponse<ProviderResponse>),
        (status = 400, description = "Invalid or unavailable provider", body = ErrorResponse)
    )
)]
pub async fn get_provider(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> Result<Json<ApiResponse<ProviderResponse>>, AppError> {
    let provider = parse_provider(&provider)?;
    let provider = state.providers.describe(provider)?;
    Ok(ok("Provider fetched successfully", provider))
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider}/markets",
    tag = "Providers",
    params(("provider" = String, Path, description = "Provider route id"), MarketListQuery),
    responses(
        (status = 200, description = "Normalized provider markets page", body = ApiResponse<MarketPageResponse>),
        (status = 400, description = "Invalid or unavailable provider", body = ErrorResponse),
        (status = 502, description = "Upstream provider error", body = ErrorResponse)
    )
)]
pub async fn fetch_markets(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<MarketListQuery>,
) -> Result<Json<ApiResponse<MarketPageResponse>>, AppError> {
    let provider = parse_provider(&provider)?;
    let markets = state.providers.fetch_markets(provider, &query).await?;
    Ok(ok("Markets fetched successfully", markets))
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider}/markets/{market_id}",
    tag = "Providers",
    params(
        ("provider" = String, Path, description = "Provider route id"),
        ("market_id" = String, Path, description = "Provider market id")
    ),
    responses(
        (status = 200, description = "Normalized provider market", body = ApiResponse<MarketResponse>),
        (status = 400, description = "Invalid provider or market", body = ErrorResponse),
        (status = 404, description = "Market not found", body = ErrorResponse),
        (status = 502, description = "Upstream provider error", body = ErrorResponse)
    )
)]
pub async fn fetch_market(
    State(state): State<AppState>,
    Path((provider, market_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<MarketResponse>>, AppError> {
    let provider = parse_provider(&provider)?;
    let market = state.providers.fetch_market(provider, &market_id).await?;
    Ok(ok("Market fetched successfully", market))
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider}/markets/{market_id}/order-book",
    tag = "Providers",
    params(
        ("provider" = String, Path, description = "Provider route id"),
        ("market_id" = String, Path, description = "Provider market id"),
        OrderBookQuery
    ),
    responses(
        (status = 200, description = "Provider normalized order book", body = ApiResponse<OrderBookResponse>),
        (status = 400, description = "Invalid provider, market, or outcome", body = ErrorResponse),
        (status = 502, description = "Upstream provider error", body = ErrorResponse)
    )
)]
pub async fn fetch_order_book(
    State(state): State<AppState>,
    Path((provider, market_id)): Path<(String, String)>,
    Query(query): Query<OrderBookQuery>,
) -> Result<Json<ApiResponse<OrderBookResponse>>, AppError> {
    let provider = parse_provider(&provider)?;
    let order_book = state
        .providers
        .fetch_order_book(provider, &market_id, &query.outcome_id)
        .await?;
    Ok(ok("Order book fetched successfully", order_book))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OrderBookQuery {
    pub outcome_id: String,
}

pub fn parse_provider(value: &str) -> Result<ProviderId, AppError> {
    ProviderId::from_str(value).map_err(|message| AppError::BadRequest(message.to_owned()))
}
