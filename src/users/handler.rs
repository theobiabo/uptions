use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app::state::AppState,
    error::{AppError, ErrorResponse},
    users::service::JoinWaitlistStruct,
};

#[derive(Deserialize, Serialize, ToSchema)]
pub struct WaitlistUser {
    #[schema(example = "ada@example.com")]
    email: String,
    #[schema(example = "Ada Lovelace")]
    name: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/users/waitlist",
    tag = "Users",
    request_body = WaitlistUser,
    responses(
        (status = 201, description = "User joined the waitlist"),
        (status = 500, description = "Database failure", body = ErrorResponse)
    )
)]
pub async fn join_waitlist(
    State(state): State<AppState>,
    Json(payload): Json<WaitlistUser>,
) -> Result<StatusCode, AppError> {
    state
        .user_service
        .join_waitlist(JoinWaitlistStruct {
            name: payload.name,
            email: payload.email,
        })
        .await?;

    Ok(StatusCode::CREATED)
}
