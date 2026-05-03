use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::Value;

use crate::{
    app::state::AppState,
    error::AppError,
    polymarket::dto::{MarketsQuery, PolymarketAuthRequest, PolymarketAuthResponse},
};

pub async fn authenticate_polymarket(
    State(state): State<AppState>,
    Json(payload): Json<PolymarketAuthRequest>,
) -> Result<Json<PolymarketAuthResponse>, AppError> {
    let response = state
        .polymarket_client
        .create_or_derive_api_key(payload.nonce)
        .await?;

    Ok(Json(response))
}

pub async fn fetch_markets(
    State(state): State<AppState>,
    Query(query): Query<MarketsQuery>,
) -> Result<Json<Value>, AppError> {
    let markets = state.polymarket_client.fetch_markets(&query).await?;

    Ok(Json(markets))
}
