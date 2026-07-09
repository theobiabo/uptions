use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
};
use serde_json::{Value, json};

use crate::{app::state::AppState, mcp::service::handle_message};

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
