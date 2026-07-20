use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app::state::AppState,
    auth::{
        dto::{WalletChallengeRequest, WalletChallengeResponse},
        handlers::bearer_token,
    },
    error::{AppError, ErrorResponse},
    providers::types::{Chain, ChainId, ProviderId},
    response::{ApiResponse, created, ok},
    users::service::JoinWaitlistStruct,
};

#[derive(Deserialize, Serialize, ToSchema)]
pub struct UpdateTradingProviderRequest {
    pub provider: ProviderId,
}

#[derive(Serialize, ToSchema)]
pub struct UserTradingProviderResponse {
    pub preferred_trading_provider: ProviderId,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct UpdateWalletRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    pub provider: ProviderId,
    pub chain: Chain,
    #[schema(example = 137)]
    pub chain_id: ChainId,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub nonce: String,
    #[schema(
        example = "0x5f2c9c0d93b1b3fddc55c4f98ccf5281af2c0612fd4f2cfd2c7d4dd4f3838f620dcf54e02db91f7df0ec6ee25b9e6f74fd839cc13a5d08d64f6b3db2de4d6c881b"
    )]
    pub signature: String,
}

#[derive(Serialize, ToSchema)]
pub struct UserWalletResponse {
    pub chain: Chain,
    pub chain_id: ChainId,
    pub provider: ProviderId,
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
    state.providers.adapter(payload.provider)?;
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
    post,
    path = "/api/v1/users/wallet/challenge",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = WalletChallengeRequest,
    responses(
        (status = 200, description = "Purpose-bound wallet association challenge created", body = ApiResponse<WalletChallengeResponse>),
        (status = 400, description = "Invalid wallet or non-Polygon chain", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 409, description = "Wallet belongs to another account", body = ErrorResponse)
    )
)]
pub async fn create_wallet_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<WalletChallengeRequest>,
) -> Result<Json<ApiResponse<WalletChallengeResponse>>, AppError> {
    let access_token = bearer_token(&headers)?;
    let challenge = state
        .auth_service
        .create_wallet_challenge(
            &access_token,
            &payload.wallet_address,
            payload.provider,
            payload.chain,
            payload.chain_id,
        )
        .await?;

    Ok(ok("Wallet challenge created successfully", challenge))
}

#[utoipa::path(
    patch,
    path = "/api/v1/users/wallet",
    tag = "Users",
    security(("bearer_auth" = [])),
    request_body = UpdateWalletRequest,
    responses(
        (status = 200, description = "Verified wallet associated", body = ApiResponse<UserWalletResponse>),
        (status = 400, description = "Invalid, expired, or used challenge", body = ErrorResponse),
        (status = 401, description = "Missing bearer token or invalid signature", body = ErrorResponse),
        (status = 409, description = "Wallet belongs to another account", body = ErrorResponse)
    )
)]
pub async fn update_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateWalletRequest>,
) -> Result<Json<ApiResponse<UserWalletResponse>>, AppError> {
    let expected_chain = state.providers.chain(payload.provider)?;
    if payload.chain != expected_chain || payload.chain_id != expected_chain.id() {
        return Err(AppError::ProviderValidation {
            code: "PROVIDER_CHAIN_MISMATCH",
            message: "wallet chain is not supported by selected provider".to_owned(),
        });
    }
    let access_token = bearer_token(&headers)?;
    let wallet_address = state
        .auth_service
        .associate_wallet(
            &access_token,
            &payload.wallet_address,
            payload.provider,
            payload.chain,
            payload.chain_id,
            &payload.nonce,
            &payload.signature,
        )
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

#[cfg(test)]
mod tests {
    use super::UpdateWalletRequest;

    #[test]
    fn wallet_update_requires_nonce_and_signature() {
        let payload = serde_json::json!({
            "wallet_address": "0x1111111111111111111111111111111111111111",
            "provider": "POLYMARKET",
            "chain": "POLYGON",
            "chain_id": 137
        });

        assert!(serde_json::from_value::<UpdateWalletRequest>(payload).is_err());
    }
}
