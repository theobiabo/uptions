use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Unauthorized")]
    Unauthorized,

    #[error("Email already exists")]
    EmailExists,

    #[error("Invalid email or password")]
    InvalidCredentials,

    #[error("Database error")]
    DatabaseError,

    #[error("External API error")]
    ExternalApiError,

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::EmailExists => StatusCode::CONFLICT,
            AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ExternalApiError => StatusCode::BAD_GATEWAY,
            AppError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(json!({
            "success": false,
            "message": self.to_string()
        }));

        (status, body).into_response()
    }
}
