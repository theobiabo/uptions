pub mod docs;
pub mod state;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::docs::{openapi_json, swagger_ui},
    app::state::AppState,
    auth::handlers::{create_challenge, current_user, verify_challenge},
    polymarket::handlers::fetch_markets,
    users::handler::join_waitlist,
};

async fn health_check() -> &'static str {
    "Uptions endpoint is running"
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
        .with_state(state)
}
