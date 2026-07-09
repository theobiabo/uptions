use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
};
use serde_json::{Value, json};

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    error::{AppError, ErrorResponse},
    mcp::{
        dto::{
            McpApprovalDecisionResponse, McpApprovalResponse, McpJsonRpcRequest, McpJsonRpcResponse,
        },
        service::{approve_request, get_approval, handle_message, list_approvals, reject_request},
    },
    response::{ApiResponse, ok},
};

#[utoipa::path(
    get,
    path = "/api/v1/mcp/approvals",
    tag = "MCP",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "MCP approval requests", body = ApiResponse<Vec<McpApprovalResponse>>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn list_mcp_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<McpApprovalResponse>>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let approvals = list_approvals(&state, &user_id).await?;
    Ok(ok("MCP approval requests fetched successfully", approvals))
}

#[utoipa::path(
    get,
    path = "/api/v1/mcp/approvals/{approval_id}",
    tag = "MCP",
    security(("bearer_auth" = [])),
    params(("approval_id" = String, Path, description = "MCP approval request id")),
    responses(
        (status = 200, description = "MCP approval request", body = ApiResponse<McpApprovalResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "MCP approval request not found", body = ErrorResponse)
    )
)]
pub async fn get_mcp_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
) -> Result<Json<ApiResponse<McpApprovalResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let approval = get_approval(&state, &user_id, &approval_id).await?;
    Ok(ok("MCP approval request fetched successfully", approval))
}

#[utoipa::path(
    post,
    path = "/api/v1/mcp/approvals/{approval_id}/approve",
    tag = "MCP",
    security(("bearer_auth" = [])),
    params(("approval_id" = String, Path, description = "MCP approval request id")),
    responses(
        (status = 200, description = "MCP approval request approved", body = ApiResponse<McpApprovalDecisionResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "MCP approval request not found", body = ErrorResponse),
        (status = 409, description = "MCP approval request is not pending", body = ErrorResponse)
    )
)]
pub async fn approve_mcp_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
) -> Result<Json<ApiResponse<McpApprovalDecisionResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let decision = approve_request(&state, &user_id, &approval_id).await?;
    Ok(ok("MCP approval request approved", decision))
}

#[utoipa::path(
    post,
    path = "/api/v1/mcp/approvals/{approval_id}/reject",
    tag = "MCP",
    security(("bearer_auth" = [])),
    params(("approval_id" = String, Path, description = "MCP approval request id")),
    responses(
        (status = 200, description = "MCP approval request rejected", body = ApiResponse<McpApprovalDecisionResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "MCP approval request not found", body = ErrorResponse),
        (status = 409, description = "MCP approval request is not pending", body = ErrorResponse)
    )
)]
pub async fn reject_mcp_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
) -> Result<Json<ApiResponse<McpApprovalDecisionResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let decision = reject_request(&state, &user_id, &approval_id).await?;
    Ok(ok("MCP approval request rejected", decision))
}

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

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}
