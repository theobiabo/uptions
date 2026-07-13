use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    analytics::dto::AnalyticsOverviewResponse,
    app::state::AppState,
    auth::handlers::bearer_token,
    error::{AppError, ErrorResponse},
    response::{ApiResponse, ok},
};

#[utoipa::path(
    get,
    path = "/api/v1/analytics/overview",
    tag = "Analytics",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authenticated user's persisted analytics overview", body = ApiResponse<AnalyticsOverviewResponse>),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn analytics_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<AnalyticsOverviewResponse>>, AppError> {
    let access_token = bearer_token(&headers)?;
    let user_id = state.auth_service.current_user_id(&access_token).await?;
    let overview = state.analytics_service.overview(&user_id).await?;

    Ok(ok("Analytics overview fetched successfully", overview))
}
