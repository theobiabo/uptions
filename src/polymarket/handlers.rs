use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::Value;

use crate::{app::state::AppState, error::AppError, polymarket::dto::MarketsQuery};

pub async fn fetch_markets(
    State(state): State<AppState>,
    Query(query): Query<MarketsQuery>,
) -> Result<Json<Value>, AppError> {
    let markets = state.polymarket_client.fetch_markets(&query).await?;

    Ok(Json(markets))
}
