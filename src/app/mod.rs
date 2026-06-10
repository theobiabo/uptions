pub mod docs;
pub mod state;

use axum::{
    Json, Router,
    http::{HeaderValue, Method},
    routing::{get, post},
};
use tower_http::{
    cors::{AllowHeaders, AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use crate::{
    app::docs::{openapi_json, swagger_ui},
    app::state::AppState,
    auth::handlers::{
        connect_polymarket, create_challenge, current_user, forgot_password, login, reset_password,
        signup, verify_challenge, verify_email,
    },
    polymarket::handlers::fetch_markets,
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
        .route("/polymarket/markets", get(fetch_markets))
        .route("/users/waitlist", post(join_waitlist))
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
        .route("/docs", get(swagger_ui))
        .route("/docs/openapi.json", get(openapi_json))
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
