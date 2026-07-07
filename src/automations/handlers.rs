use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    automations::dto::{
        AutomationAlertResponse, AutomationResponse, PublishAutomationRequest,
        TestRunAutomationRequest, TestRunAutomationResponse,
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
