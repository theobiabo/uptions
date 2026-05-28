use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app::state::AppState,
    error::{AppError, ErrorResponse},
    response::{ApiResponse, created},
    users::service::JoinWaitlistStruct,
};

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
    post,
    path = "/api/v1/users/waitlist",
    tag = "Users",
    request_body = WaitlistUser,
    responses(
        (status = 201, description = "User joined the waitlist", body = ApiResponse<WaitlistResponse>),
        (status = 500, description = "Database failure", body = ErrorResponse)
    )
)]
pub async fn join_waitlist(
    State(state): State<AppState>,
    Json(payload): Json<WaitlistUser>,
) -> Result<(StatusCode, Json<ApiResponse<WaitlistResponse>>), AppError> {
    let email = payload.email;

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
