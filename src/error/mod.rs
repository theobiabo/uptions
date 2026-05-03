use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    ExternalApiError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::ExternalApiError(_) => StatusCode::BAD_GATEWAY,
            AppError::ConfigurationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(json!({
            "success": false,
            "message": self.to_string()
        }));

        (status, body).into_response()
    }
}
