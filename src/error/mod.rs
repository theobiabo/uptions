use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::json;
use tracing::error;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    #[schema(example = false)]
    pub success: bool,
    #[schema(example = "Invalid request")]
    pub message: String,
    #[schema(example = "PROVIDER_INSTRUMENT_MISMATCH")]
    pub code: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Unauthorized")]
    Unauthorized,

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    ExternalApiError(String),

    #[error("{message}")]
    ProviderValidation { code: &'static str, message: String },

    #[error("{0}")]
    DatabaseError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::BadRequest(_) | AppError::ProviderValidation { .. } => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            AppError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::ExternalApiError(detail) => {
                error!(error = %detail, "external API request failed");
                (
                    StatusCode::BAD_GATEWAY,
                    "Upstream service unavailable".to_owned(),
                )
            }
            AppError::DatabaseError(detail) => {
                error!(error = %detail, "internal database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_owned(),
                )
            }
        };

        let code = match &self {
            AppError::ProviderValidation { code, .. } => Some(*code),
            _ => None,
        };
        let body = Json(json!({
            "success": false,
            "message": message,
            "code": code
        }));

        (status, body).into_response()
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::DatabaseError(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};
    use serde_json::Value;

    use super::AppError;

    #[tokio::test]
    async fn provider_validation_errors_have_stable_codes() {
        let response = AppError::ProviderValidation {
            code: "PROVIDER_CHAIN_MISMATCH",
            message: "resolved chain does not match provider".to_owned(),
        }
        .into_response();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["code"], "PROVIDER_CHAIN_MISMATCH");
        assert_eq!(payload["message"], "resolved chain does not match provider");
    }

    #[tokio::test]
    async fn sanitizes_database_errors() {
        let response = AppError::DatabaseError("postgres secret detail".to_owned()).into_response();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["message"], "Internal server error");
        assert!(!String::from_utf8_lossy(&body).contains("postgres secret detail"));
    }

    #[tokio::test]
    async fn sanitizes_external_errors() {
        let response =
            AppError::ExternalApiError("upstream secret detail".to_owned()).into_response();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["message"], "Upstream service unavailable");
        assert!(!String::from_utf8_lossy(&body).contains("upstream secret detail"));
    }
}
