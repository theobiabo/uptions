use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header},
};

use crate::{
    app::state::AppState,
    auth::dto::{
        AuthUserResponse, CreateChallengeRequest, CreateChallengeResponse, VerifyChallengeRequest,
        VerifyChallengeResponse,
    },
    error::{AppError, ErrorResponse},
    response::{ApiResponse, ok},
};

#[utoipa::path(
    post,
    path = "/api/v1/auth/challenge",
    tag = "Auth",
    request_body = CreateChallengeRequest,
    responses(
        (status = 200, description = "Challenge created successfully", body = ApiResponse<CreateChallengeResponse>),
        (status = 400, description = "Invalid wallet address", body = ErrorResponse),
        (status = 500, description = "Server or configuration failure", body = ErrorResponse)
    )
)]
pub async fn create_challenge(
    State(state): State<AppState>,
    Json(payload): Json<CreateChallengeRequest>,
) -> Result<Json<ApiResponse<CreateChallengeResponse>>, AppError> {
    let response = state
        .auth_service
        .create_challenge(&payload.wallet_address)
        .await?;

    Ok(ok("Challenge created successfully", response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/verify",
    tag = "Auth",
    request_body = VerifyChallengeRequest,
    responses(
        (status = 200, description = "Wallet verified and session issued", body = ApiResponse<VerifyChallengeResponse>),
        (status = 400, description = "Invalid or expired challenge", body = ErrorResponse),
        (status = 401, description = "Invalid signature", body = ErrorResponse)
    )
)]
pub async fn verify_challenge(
    State(state): State<AppState>,
    Json(payload): Json<VerifyChallengeRequest>,
) -> Result<Json<ApiResponse<VerifyChallengeResponse>>, AppError> {
    let response = state
        .auth_service
        .verify_challenge(&payload.wallet_address, &payload.signature)
        .await?;

    Ok(ok("Wallet verified successfully", response))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "Auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current authenticated user", body = ApiResponse<AuthUserResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<AuthUserResponse>>, AppError> {
    let access_token = bearer_token(&headers)?;
    let user = state.auth_service.current_user(&access_token).await?;

    Ok(ok("Current user fetched successfully", user))
}

fn bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let header_value = headers
        .get(header::AUTHORIZATION)
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?;

    let token = header_value
        .strip_prefix("Bearer ")
        .or_else(|| header_value.strip_prefix("bearer "))
        .ok_or(AppError::Unauthorized)?;

    if token.is_empty() {
        return Err(AppError::Unauthorized);
    }

    Ok(token.to_owned())
}
