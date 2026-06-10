use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header},
};

use crate::{
    app::state::AppState,
    auth::dto::{
        AuthSessionResponse, AuthUserResponse, ConnectPolymarketRequest, CreateChallengeRequest,
        CreateChallengeResponse, ForgotPasswordRequest, LoginRequest, ResetPasswordRequest,
        SignupRequest, VenueConnectionResponse, VerifyChallengeRequest, VerifyChallengeResponse,
        VerifyEmailRequest,
    },
    error::{AppError, ErrorResponse},
    response::{ApiResponse, ok},
};

#[utoipa::path(
    post,
    path = "/api/v1/auth/signup",
    tag = "Auth",
    request_body = SignupRequest,
    responses(
        (status = 200, description = "Account created and verification email sent", body = ApiResponse<AuthUserResponse>),
        (status = 400, description = "Invalid signup payload", body = ErrorResponse),
        (status = 409, description = "Email already registered", body = ErrorResponse)
    )
)]
pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<Json<ApiResponse<AuthUserResponse>>, AppError> {
    let response = state.auth_service.signup(payload).await?;

    Ok(ok(
        "Account created. Check your email to verify it.",
        response,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Session issued", body = ApiResponse<AuthSessionResponse>),
        (status = 400, description = "Invalid login payload", body = ErrorResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<ApiResponse<AuthSessionResponse>>, AppError> {
    let response = state.auth_service.login(payload).await?;

    Ok(ok("Logged in successfully", response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/verify-email",
    tag = "Auth",
    request_body = VerifyEmailRequest,
    responses(
        (status = 200, description = "Email verified and session issued", body = ApiResponse<AuthSessionResponse>),
        (status = 400, description = "Invalid or expired verification token", body = ErrorResponse)
    )
)]
pub async fn verify_email(
    State(state): State<AppState>,
    Json(payload): Json<VerifyEmailRequest>,
) -> Result<Json<ApiResponse<AuthSessionResponse>>, AppError> {
    let response = state.auth_service.verify_email(&payload.token).await?;

    Ok(ok("Email verified successfully", response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/forgot-password",
    tag = "Auth",
    request_body = ForgotPasswordRequest,
    responses(
        (status = 200, description = "Password reset email sent when account exists", body = ApiResponse<String>),
        (status = 400, description = "Invalid forgot password payload", body = ErrorResponse)
    )
)]
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    state.auth_service.forgot_password(payload).await?;

    Ok(ok(
        "If an account exists for that email, a reset link has been sent.",
        "ok".to_owned(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/reset-password",
    tag = "Auth",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset and session issued", body = ApiResponse<AuthSessionResponse>),
        (status = 400, description = "Invalid or expired reset token", body = ErrorResponse)
    )
)]
pub async fn reset_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<ApiResponse<AuthSessionResponse>>, AppError> {
    let response = state.auth_service.reset_password(payload).await?;

    Ok(ok("Password reset successfully", response))
}

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

#[utoipa::path(
    post,
    path = "/api/v1/venue-connections/polymarket",
    tag = "Venue Connections",
    security(("bearer_auth" = [])),
    request_body = ConnectPolymarketRequest,
    responses(
        (status = 200, description = "Polymarket connection saved", body = ApiResponse<VenueConnectionResponse>),
        (status = 400, description = "Invalid Polymarket connection payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn connect_polymarket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConnectPolymarketRequest>,
) -> Result<Json<ApiResponse<VenueConnectionResponse>>, AppError> {
    let access_token = bearer_token(&headers)?;
    let connection = state
        .auth_service
        .connect_polymarket(&access_token, payload)
        .await?;

    Ok(ok("Polymarket connection saved successfully", connection))
}

pub fn bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
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
