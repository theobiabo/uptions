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
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::ExternalApiError(_) => StatusCode::BAD_GATEWAY,
        };

        let body = Json(json!({
            "success": false,
            "message": self.to_string()
        }));

        (status, body).into_response()
    }
}
