pub mod docs;
pub mod state;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app::docs::{openapi_json, swagger_ui},
    app::state::AppState,
    polymarket::handlers::{authenticate_polymarket, fetch_markets},
};

async fn health_check() -> &'static str {
    "Uptions endpoint is running"
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(health_check))
        .route("/polymarket/auth", post(authenticate_polymarket))
        .route("/polymarket/markets", get(fetch_markets))
        .route("/docs", get(swagger_ui))
        .route("/docs/openapi.json", get(openapi_json))
        .with_state(state)
}
