use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, header::HeaderName},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    automations::dto::{
        AutomationAlertResponse, AutomationResponse, ClearAlertsResponse, MarkAlertsReadResponse,
        PublishAutomationRequest, TestRunAutomationRequest, TestRunAutomationResponse,
        UpdateAutomationStatusRequest,
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
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "Optional UUID used to replay a previous publish for this user")
    ),
    request_body = PublishAutomationRequest,
    responses(
        (status = 200, description = "Automation published successfully or replayed", body = ApiResponse<AutomationResponse>),
        (status = 400, description = "Invalid automation payload", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn publish_automation(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Json<ApiResponse<AutomationResponse>>, AppError> {
    let payload: PublishAutomationRequest = parse_payload(payload, "Invalid automation payload")?;
    let user_id = authenticated_user_id(&state, &headers).await?;
    let idempotency_key = idempotency_key(&headers)?;
    let automation = state
        .automation_service
        .publish(&user_id, payload, idempotency_key)
        .await?;
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
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Json<ApiResponse<AutomationResponse>>, AppError> {
    let payload: PublishAutomationRequest = parse_payload(payload, "Invalid automation payload")?;
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
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Json<ApiResponse<AutomationResponse>>, AppError> {
    let payload: UpdateAutomationStatusRequest =
        parse_payload(payload, "Invalid automation status payload")?;
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
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Json<ApiResponse<TestRunAutomationResponse>>, AppError> {
    let payload: TestRunAutomationRequest = parse_payload(payload, "Invalid test run payload")?;
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

#[utoipa::path(
    delete,
    path = "/api/v1/automation-alerts",
    tag = "Builder",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Automation alerts cleared", body = ApiResponse<ClearAlertsResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn clear_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<ClearAlertsResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let deleted = state.automation_service.clear_alerts(&user_id).await?;

    Ok(ok(
        "Automation alerts cleared",
        ClearAlertsResponse { deleted },
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/automation-alerts/read",
    tag = "Builder",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Automation alerts marked as read", body = ApiResponse<MarkAlertsReadResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn mark_alerts_read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<MarkAlertsReadResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let updated = state.automation_service.mark_alerts_read(&user_id).await?;

    Ok(ok(
        "Automation alerts marked as read",
        MarkAlertsReadResponse { updated },
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/automation-alerts/{alert_id}/read",
    tag = "Builder",
    security(("bearer_auth" = [])),
    params(
        ("alert_id" = String, Path, description = "Automation alert id")
    ),
    responses(
        (status = 200, description = "Automation alert marked as read", body = ApiResponse<AutomationAlertResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse),
        (status = 404, description = "Automation alert not found", body = ErrorResponse)
    )
)]
pub async fn mark_alert_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(alert_id): Path<String>,
) -> Result<Json<ApiResponse<AutomationAlertResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let alert = state
        .automation_service
        .mark_alert_read(&user_id, &alert_id)
        .await?;

    Ok(ok("Automation alert marked as read", alert))
}

fn parse_payload<T>(
    payload: Result<Json<Value>, JsonRejection>,
    message: &str,
) -> Result<T, AppError>
where
    T: DeserializeOwned,
{
    let value = payload
        .map(|Json(payload)| payload)
        .map_err(|error| AppError::BadRequest(format!("{message}: {}", error.body_text())))?;

    serde_json::from_value(value)
        .map_err(|error| AppError::BadRequest(format!("{message}: {error}")))
}

fn idempotency_key(headers: &HeaderMap) -> Result<Option<String>, AppError> {
    let name = HeaderName::from_static("idempotency-key");
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| AppError::BadRequest("Idempotency-Key must be a valid UUID".to_owned()))?;
    let key = Uuid::parse_str(value)
        .map_err(|_| AppError::BadRequest("Idempotency-Key must be a valid UUID".to_owned()))?;
    Ok(Some(key.to_string()))
}

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::idempotency_key;

    #[test]
    fn allows_missing_idempotency_key() {
        assert_eq!(idempotency_key(&HeaderMap::new()).unwrap(), None);
    }

    #[test]
    fn parses_and_normalizes_uuid_idempotency_key() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            HeaderValue::from_static("550E8400-E29B-41D4-A716-446655440000"),
        );

        assert_eq!(
            idempotency_key(&headers).unwrap().as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
    }

    #[test]
    fn rejects_non_uuid_idempotency_key() {
        let mut headers = HeaderMap::new();
        headers.insert("idempotency-key", HeaderValue::from_static("retry-1"));

        assert!(idempotency_key(&headers).is_err());
    }
}
