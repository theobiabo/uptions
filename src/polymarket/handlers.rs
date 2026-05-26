use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::Value;

use crate::{
    app::state::AppState,
    error::{AppError, ErrorResponse},
    polymarket::dto::MarketsQuery,
};

#[utoipa::path(
    get,
    path = "/api/v1/polymarket/markets",
    tag = "Polymarket",
    params(MarketsQuery),
    responses(
        (status = 200, description = "Raw Polymarket markets payload", body = Value),
        (status = 502, description = "Upstream Polymarket error", body = ErrorResponse)
    )
)]
pub async fn fetch_markets(
    State(state): State<AppState>,
    Query(query): Query<MarketsQuery>,
) -> Result<Json<Value>, AppError> {
    let markets = state.polymarket_client.fetch_markets(&query).await?;

    Ok(Json(markets))
}
