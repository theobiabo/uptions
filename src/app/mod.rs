pub mod docs;
pub mod state;

use axum::{
    Json, Router,
    http::{HeaderValue, Method},
    routing::{get, patch, post, put},
};
use tower_http::{
    cors::{AllowHeaders, AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use crate::{
    app::docs::swagger_ui,
    app::state::AppState,
    auth::handlers::{
        connect_polymarket, create_challenge, current_user, forgot_password, login, reset_password,
        signup, verify_challenge, verify_email,
    },
    automations::handlers::{
        clear_alerts, delete_automation, list_alerts, list_automations, mark_alert_read,
        mark_alerts_read, publish_automation, test_run_automation, update_automation,
        update_automation_status,
    },
    mcp::handlers::{
        approve_mcp_approval, get_mcp_approval, handle_mcp, list_mcp_approvals, reject_mcp_approval,
    },
    notifications::handlers::stream_alerts,
    polymarket::handlers::{fetch_market, fetch_markets},
    response::{ApiResponse, ok},
    users::handler::join_waitlist,
};

#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "Health",
    responses(
        (status = 200, description = "Application is healthy", body = ApiResponse<String>)
    )
)]
async fn health_check() -> Json<ApiResponse<&'static str>> {
    ok("Application is healthy", "Uptions endpoint is running")
}

fn api_v1_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/verify-email", post(verify_email))
        .route("/auth/forgot-password", post(forgot_password))
        .route("/auth/reset-password", post(reset_password))
        .route("/auth/challenge", post(create_challenge))
        .route("/auth/verify", post(verify_challenge))
        .route("/auth/me", get(current_user))
        .route("/venue-connections/polymarket", post(connect_polymarket))
        .route(
            "/automations",
            get(list_automations).post(publish_automation),
        )
        .route(
            "/automations/{automation_id}",
            put(update_automation).delete(delete_automation),
        )
        .route(
            "/automations/{automation_id}/status",
            patch(update_automation_status),
        )
        .route("/automations/test-run", post(test_run_automation))
        .route("/automation-alerts", get(list_alerts).delete(clear_alerts))
        .route("/automation-alerts/read", patch(mark_alerts_read))
        .route("/automation-alerts/{alert_id}/read", patch(mark_alert_read))
        .route("/automation-alerts/stream", get(stream_alerts))
        .route("/polymarket/markets", get(fetch_markets))
        .route("/polymarket/markets/{market_id}", get(fetch_market))
        .route("/users/waitlist", post(join_waitlist))
        .route("/mcp", post(handle_mcp))
        .route("/mcp/approvals", get(list_mcp_approvals))
        .route("/mcp/approvals/{approval_id}", get(get_mcp_approval))
        .route(
            "/mcp/approvals/{approval_id}/approve",
            post(approve_mcp_approval),
        )
        .route(
            "/mcp/approvals/{approval_id}/reject",
            post(reject_mcp_approval),
        )
}

fn is_allowed_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    if origin == "https://www.uptions.xyz" {
        return true;
    }

    let Some(host_start) = origin.find("://").map(|index| index + 3) else {
        return false;
    };

    let host_and_port = origin[host_start..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();

    let host = host_and_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_matches('[')
        .trim_matches(']');

    host.eq_ignore_ascii_case("localhost")
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _request_parts| {
            is_allowed_origin(origin)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::mirror_request())
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(health_check))
        .route("/mcp", post(handle_mcp))
        .merge(swagger_ui())
        .nest("/api/v1", api_v1_router())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(cors_layer())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::is_allowed_origin;
    use axum::http::HeaderValue;

    #[test]
    fn allows_configured_production_origin() {
        assert!(is_allowed_origin(&HeaderValue::from_static(
            "https://www.uptions.xyz",
        )));
    }

    #[test]
    fn allows_localhost_on_any_port() {
        assert!(is_allowed_origin(&HeaderValue::from_static(
            "http://localhost:5173",
        )));
        assert!(is_allowed_origin(&HeaderValue::from_static(
            "https://localhost:3000",
        )));
    }

    #[test]
    fn rejects_other_origins() {
        assert!(!is_allowed_origin(&HeaderValue::from_static(
            "https://uptions.xyz",
        )));
        assert!(!is_allowed_origin(&HeaderValue::from_static(
            "https://example.com",
        )));
    }
}
