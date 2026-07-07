use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    automations::dto::{
        AutomationAlertResponse, AutomationResponse, PublishAutomationRequest,
        TestRunAutomationRequest, TestRunAutomationResponse, UpdateAutomationStatusRequest,
    },
    error::{AppError, ErrorResponse},
    response::{ApiResponse, ok},
};

#[utoipa::path(
    get,
    path = "/api/v1/automations",
    tag = "Builder",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authenticated user automations", body = ApiResponse<Vec<AutomationResponse>>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn list_automations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<AutomationResponse>>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let automations = state.automation_service.list(&user_id).await?;
    Ok(ok("Automations fetched successfully", automations))
}

#[utoipa::path(
    post,
    path = "/api/v1/automations",
    tag = "Builder",
    security(("bearer_auth" = [])),
    request_body = PublishAutomationRequest,
    responses(
        (status = 200, description = "Automation published successfully", body = ApiResponse<AutomationResponse>),
        (status = 400, description = "Invalid automation payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn publish_automation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PublishAutomationRequest>,
) -> Result<Json<ApiResponse<AutomationResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let automation = state.automation_service.publish(&user_id, payload).await?;
    Ok(ok("Automation published successfully", automation))
}

#[utoipa::path(
    put,
    path = "/api/v1/automations/{automation_id}",
    tag = "Builder",
    security(("bearer_auth" = [])),
    params(
        ("automation_id" = String, Path, description = "Automation id")
    ),
    request_body = PublishAutomationRequest,
    responses(
        (status = 200, description = "Automation updated successfully", body = ApiResponse<AutomationResponse>),
        (status = 400, description = "Invalid automation payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "Automation not found", body = ErrorResponse)
    )
)]
pub async fn update_automation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(automation_id): Path<String>,
    Json(payload): Json<PublishAutomationRequest>,
) -> Result<Json<ApiResponse<AutomationResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let automation = state
        .automation_service
        .update(&user_id, &automation_id, payload)
        .await?;
    Ok(ok("Automation updated successfully", automation))
}

#[utoipa::path(
    patch,
    path = "/api/v1/automations/{automation_id}/status",
    tag = "Builder",
    security(("bearer_auth" = [])),
    params(
        ("automation_id" = String, Path, description = "Automation id")
    ),
    request_body = UpdateAutomationStatusRequest,
    responses(
        (status = 200, description = "Automation status updated", body = ApiResponse<AutomationResponse>),
        (status = 400, description = "Invalid automation status", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "Automation not found", body = ErrorResponse)
    )
)]
pub async fn update_automation_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(automation_id): Path<String>,
    Json(payload): Json<UpdateAutomationStatusRequest>,
) -> Result<Json<ApiResponse<AutomationResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let automation = state
        .automation_service
        .set_status(&user_id, &automation_id, payload.status)
        .await?;
    Ok(ok("Automation status updated", automation))
}

#[utoipa::path(
    delete,
    path = "/api/v1/automations/{automation_id}",
    tag = "Builder",
    security(("bearer_auth" = [])),
    params(
        ("automation_id" = String, Path, description = "Automation id")
    ),
    responses(
        (status = 200, description = "Automation deleted successfully", body = ApiResponse<String>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "Automation not found", body = ErrorResponse)
    )
)]
pub async fn delete_automation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(automation_id): Path<String>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    state
        .automation_service
        .delete(&user_id, &automation_id)
        .await?;
    Ok(ok("Automation deleted successfully", "ok".to_owned()))
}

#[utoipa::path(
    post,
    path = "/api/v1/automations/test-run",
    tag = "Builder",
    security(("bearer_auth" = [])),
    request_body = TestRunAutomationRequest,
    responses(
        (status = 200, description = "Automation test run completed", body = ApiResponse<TestRunAutomationResponse>),
        (status = 400, description = "Invalid workflow payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn test_run_automation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<TestRunAutomationRequest>,
) -> Result<Json<ApiResponse<TestRunAutomationResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let result = state.automation_service.test_run(&user_id, payload).await?;
    Ok(ok("Automation test run completed", result))
}

#[utoipa::path(
    get,
    path = "/api/v1/automation-alerts",
    tag = "Builder",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Recent automation alerts", body = ApiResponse<Vec<AutomationAlertResponse>>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn list_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<AutomationAlertResponse>>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let alerts = state.automation_service.alerts(&user_id).await?;
    Ok(ok("Automation alerts fetched successfully", alerts))
}

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}
