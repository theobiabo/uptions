pub mod docs;
pub mod state;

use axum::{
    Json, Router,
    routing::{get, post},
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::{
    app::docs::{openapi_json, swagger_ui},
    app::state::AppState,
    auth::handlers::{create_challenge, current_user, verify_challenge},
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
        .route("/auth/challenge", post(create_challenge))
        .route("/auth/verify", post(verify_challenge))
        .route("/auth/me", get(current_user))
        .route("/polymarket/markets", get(fetch_markets))
        .route("/users/waitlist", post(join_waitlist))
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
        .with_state(state)
}
