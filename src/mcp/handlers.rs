use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
};
use serde_json::{Value, json};

use crate::{
    app::state::AppState,
    error::ErrorResponse,
    mcp::{
        dto::{McpJsonRpcRequest, McpJsonRpcResponse},
        service::handle_message,
    },
};

#[utoipa::path(
    post,
    path = "/api/v1/mcp",
    tag = "MCP",
    request_body = McpJsonRpcRequest,
    responses(
        (status = 200, description = "MCP JSON-RPC response", body = McpJsonRpcResponse),
        (status = 400, description = "Invalid JSON-RPC payload", body = ErrorResponse)
    )
)]
pub async fn handle_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Json<Value> {
    match payload {
        Ok(Json(message)) => Json(handle_message(&state, &headers, message).await),
        Err(error) => Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": {
                "code": -32700,
                "message": format!("Invalid JSON-RPC payload: {}", error.body_text())
            }
        })),
    }
}
