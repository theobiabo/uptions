use axum::{Json, http::StatusCode};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub message: String,
    pub data: T,
    pub status_code: u16,
}

impl<T> ApiResponse<T> {
    pub fn new(message: impl Into<String>, data: T, status: StatusCode) -> Self {
        Self {
            message: message.into(),
            data,
            status_code: status.as_u16(),
        }
    }
}

pub fn ok<T>(message: impl Into<String>, data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse::new(message, data, StatusCode::OK))
}

pub fn created<T>(message: impl Into<String>, data: T) -> (StatusCode, Json<ApiResponse<T>>) {
    let status = StatusCode::CREATED;

    (status, Json(ApiResponse::new(message, data, status)))
}
