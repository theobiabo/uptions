use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    error::{AppError, ErrorResponse},
    response::{ApiResponse, created, ok},
    users::service::JoinWaitlistStruct,
    venue::{SupportedChain, SupportedVenue},
};

#[derive(Deserialize, Serialize, ToSchema)]
pub struct UpdateTradingProviderRequest {
    pub provider: SupportedVenue,
}

#[derive(Serialize, ToSchema)]
pub struct TradingProviderResponse {
    pub available: bool,
    pub chain: SupportedChain,
    pub chain_id: u64,
    pub chain_label: String,
    pub description: String,
    pub image_key: String,
    pub label: String,
    pub provider: SupportedVenue,
    pub venue_id: String,
}

#[derive(Serialize, ToSchema)]
pub struct UserTradingProviderResponse {
    pub preferred_trading_provider: SupportedVenue,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct UpdateWalletRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    pub provider: SupportedVenue,
    pub chain: SupportedChain,
    #[schema(example = 137)]
    pub chain_id: u64,
}

#[derive(Serialize, ToSchema)]
pub struct UserWalletResponse {
    pub chain: SupportedChain,
    pub chain_id: u64,
    pub provider: SupportedVenue,
    pub wallet_address: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct WaitlistUser {
    #[schema(example = "ada@example.com")]
    email: String,
}

#[derive(Serialize, ToSchema)]
pub struct WaitlistResponse {
    #[schema(example = "ada@example.com")]
    email: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/trading-providers",
    tag = "Users",
    responses(
        (status = 200, description = "Supported trading providers", body = ApiResponse<Vec<TradingProviderResponse>>)
    )
)]
pub async fn list_trading_providers() -> Json<ApiResponse<Vec<TradingProviderResponse>>> {
    let providers = SupportedVenue::all()
        .into_iter()
        .map(trading_provider_response)
        .collect();

    ok("Trading providers fetched successfully", providers)
}

#[utoipa::path(
    patch,
    path = "/api/v1/users/trading-provider",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = UpdateTradingProviderRequest,
    responses(
        (status = 200, description = "Trading provider saved", body = ApiResponse<UserTradingProviderResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn update_trading_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateTradingProviderRequest>,
) -> Result<Json<ApiResponse<UserTradingProviderResponse>>, AppError> {
    let access_token = bearer_token(&headers)?;
    let user_id = state.auth_service.current_user_id(&access_token).await?;
    let provider = state
        .user_service
        .set_preferred_trading_provider(&user_id, payload.provider)
        .await?;

    Ok(ok(
        "Trading provider saved successfully",
        UserTradingProviderResponse {
            preferred_trading_provider: provider,
        },
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/users/wallet",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = UpdateWalletRequest,
    responses(
        (status = 200, description = "Connected wallet saved", body = ApiResponse<UserWalletResponse>),
        (status = 400, description = "Invalid wallet payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn update_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateWalletRequest>,
) -> Result<Json<ApiResponse<UserWalletResponse>>, AppError> {
    let expected_chain = payload.provider.chain();

    if payload.chain != expected_chain || payload.chain_id != expected_chain.chain_id() {
        return Err(AppError::BadRequest(
            "wallet chain is not supported by selected provider".to_owned(),
        ));
    }

    let access_token = bearer_token(&headers)?;
    let user_id = state.auth_service.current_user_id(&access_token).await?;
    let wallet_address = state
        .user_service
        .set_connected_wallet(&user_id, &payload.wallet_address)
        .await?;

    Ok(ok(
        "Connected wallet saved successfully",
        UserWalletResponse {
            chain: payload.chain,
            chain_id: payload.chain_id,
            provider: payload.provider,
            wallet_address,
        },
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/users/waitlist",
    tag = "Users",
    request_body = WaitlistUser,
    responses(
        (status = 201, description = "User joined the waitlist", body = ApiResponse<WaitlistResponse>),
        (status = 409, description = "User already exists on the waitlist", body = ErrorResponse),
        (status = 500, description = "Database failure", body = ErrorResponse)
    )
)]
pub async fn join_waitlist(
    State(state): State<AppState>,
    Json(payload): Json<WaitlistUser>,
) -> Result<(StatusCode, Json<ApiResponse<WaitlistResponse>>), AppError> {
    let email = payload.email.trim().to_lowercase();

    state
        .user_service
        .join_waitlist(JoinWaitlistStruct {
            email: email.clone(),
        })
        .await?;

    Ok(created(
        "User joined the waitlist",
        WaitlistResponse { email },
    ))
}

fn trading_provider_response(provider: SupportedVenue) -> TradingProviderResponse {
    let chain = provider.chain();

    TradingProviderResponse {
        available: provider.available(),
        chain,
        chain_id: chain.chain_id(),
        chain_label: chain.label().to_owned(),
        description: provider.description().to_owned(),
        image_key: provider.image_key().to_owned(),
        label: provider.label().to_owned(),
        provider,
        venue_id: provider.id().to_owned(),
    }
}
