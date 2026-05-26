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

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    ExternalApiError(String),

    #[error("{0}")]
    DatabaseError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::ExternalApiError(_) => StatusCode::BAD_GATEWAY,
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(json!({
            "success": false,
            "message": self.to_string()
        }));

        (status, body).into_response()
    }
}

impl From<diesel::r2d2::PoolError> for AppError {
    fn from(error: diesel::r2d2::PoolError) -> Self {
        Self::DatabaseError(error.to_string())
    }
}
