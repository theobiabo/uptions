use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;

use crate::{app::state::AppState, error::AppError, users::service::JoinWaitlistStruct};

#[derive(Deserialize)]
pub struct WaitlistUser {
    email: String,
    name: String,
}

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
