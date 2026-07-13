use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    error::{AppError, ErrorResponse},
    response::{ApiResponse, ok},
    trades::dto::{
        CreateTradeIntentRequest, CreateTradeIntentResponse, ReconcileTradeResponse,
        SubmitSignedTradeRequest, SubmitSignedTradeResponse, TradeIntentResponse,
    },
};

#[utoipa::path(
    get,
    path = "/api/v1/trades",
    tag = "Trades",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authenticated user trades", body = ApiResponse<Vec<TradeIntentResponse>>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn list_trades(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<TradeIntentResponse>>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let trades = state.trade_service.list(&user_id).await?;

    Ok(ok("Trades fetched successfully", trades))
}

#[utoipa::path(
    get,
    path = "/api/v1/trades/{trade_id}",
    tag = "Trades",
    security(("bearer_auth" = [])),
    params(("trade_id" = String, Path, description = "Trade id")),
    responses(
        (status = 200, description = "Trade intent", body = ApiResponse<TradeIntentResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "Trade not found", body = ErrorResponse)
    )
)]
pub async fn get_trade(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
) -> Result<Json<ApiResponse<TradeIntentResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let trade = state.trade_service.get(&user_id, &trade_id).await?;

    Ok(ok("Trade fetched successfully", trade))
}

#[utoipa::path(
    post,
    path = "/api/v1/trades/preflight",
    tag = "Trades",
    security(("bearer_auth" = [])),
    request_body = CreateTradeIntentRequest,
    responses(
        (status = 200, description = "Trade intent prepared", body = ApiResponse<CreateTradeIntentResponse>),
        (status = 400, description = "Invalid trade payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 502, description = "Polymarket metadata error", body = ErrorResponse)
    )
)]
pub async fn create_trade_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTradeIntentRequest>,
) -> Result<Json<ApiResponse<CreateTradeIntentResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let response = state.trade_service.create_intent(&user_id, payload).await?;

    Ok(ok("Trade intent prepared successfully", response))
}

#[utoipa::path(
    post,
    path = "/api/v1/trades/{trade_id}/submit",
    tag = "Trades",
    security(("bearer_auth" = [])),
    params(("trade_id" = String, Path, description = "Trade id")),
    request_body = SubmitSignedTradeRequest,
    responses(
        (status = 200, description = "Signed trade submitted to Polymarket", body = ApiResponse<SubmitSignedTradeResponse>),
        (status = 400, description = "Invalid signed order", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 409, description = "Trade is not awaiting signature", body = ErrorResponse),
        (status = 502, description = "Polymarket order submission error", body = ErrorResponse)
    )
)]
pub async fn submit_signed_trade(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Json(payload): Json<SubmitSignedTradeRequest>,
) -> Result<Json<ApiResponse<SubmitSignedTradeResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let response = state
        .trade_service
        .submit_signed_order(&user_id, &trade_id, payload)
        .await?;

    Ok(ok("Signed trade submitted to Polymarket", response))
}

#[utoipa::path(
    post,
    path = "/api/v1/trades/{trade_id}/reconcile",
    tag = "Trades",
    security(("bearer_auth" = [])),
    params(("trade_id" = String, Path, description = "Trade id")),
    responses(
        (status = 200, description = "Trade marked or confirmed as requiring manual reconciliation", body = ApiResponse<ReconcileTradeResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 409, description = "Trade does not require reconciliation or submission is still active", body = ErrorResponse),
        (status = 404, description = "Trade not found", body = ErrorResponse)
    )
)]
pub async fn reconcile_trade(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
) -> Result<Json<ApiResponse<ReconcileTradeResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let response = state.trade_service.reconcile(&user_id, &trade_id).await?;

    Ok(ok("Trade reconciliation state checked", response))
}

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}
