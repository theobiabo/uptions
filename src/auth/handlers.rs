use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header},
};

use crate::{
    app::state::AppState,
    auth::dto::{CreateChallengeRequest, VerifyChallengeRequest},
    error::AppError,
};

pub async fn create_challenge(
    State(state): State<AppState>,
    Json(payload): Json<CreateChallengeRequest>,
) -> Result<Json<crate::auth::dto::CreateChallengeResponse>, AppError> {
    let response = state
        .auth_service
        .create_challenge(&payload.wallet_address)
        .await?;

    Ok(Json(response))
}

pub async fn verify_challenge(
    State(state): State<AppState>,
    Json(payload): Json<VerifyChallengeRequest>,
) -> Result<Json<crate::auth::dto::VerifyChallengeResponse>, AppError> {
    let response = state
        .auth_service
        .verify_challenge(&payload.wallet_address, &payload.signature)
        .await?;

    Ok(Json(response))
}

pub async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::auth::dto::AuthUserResponse>, AppError> {
    let access_token = bearer_token(&headers)?;
    let user = state.auth_service.current_user(&access_token).await?;

    Ok(Json(user))
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
