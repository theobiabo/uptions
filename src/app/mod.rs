pub mod docs;
pub mod rate_limit;
pub mod state;

use std::{net::IpAddr, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{
        HeaderName, HeaderValue, Method, Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
};
use reqwest::Url;
use sea_orm::{ConnectionTrait, Statement};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, error};

use crate::{
    analytics::handlers::analytics_overview,
    app::{docs::swagger_ui, rate_limit::RateLimiter, state::AppState},
    auth::handlers::{
        connect_polymarket, create_challenge, current_user, forgot_password, login, logout,
        logout_all, reset_password, signup, update_email, update_password, update_username,
        verify_challenge, verify_email,
    },
    automations::handlers::{
        clear_alerts, delete_automation, list_alerts, list_automations, mark_alert_read,
        mark_alerts_read, publish_automation, test_run_automation, update_automation,
        update_automation_status,
    },
    config::AppConfig,
    error::ErrorResponse,
    markets::{
        comments::handlers::{create_market_comment, list_market_comments, stream_market_comments},
        favorites::handlers::{
            favorite_market, get_market_favorite_status, list_market_favorites, unfavorite_market,
        },
    },
    mcp::handlers::{
        approve_mcp_approval, get_mcp_approval, handle_mcp, list_mcp_approvals, reject_mcp_approval,
    },
    notifications::handlers::stream_alerts,
    polymarket::handlers::{fetch_market, fetch_markets, fetch_order_book, fetch_venue_chain},
    response::{ApiResponse, ok},
    trades::handlers::{
        cancel_all_trades, cancel_market_trades, cancel_multiple_trades, cancel_trade,
        create_trade_intent, get_trade, list_trades, reconcile_trade, submit_signed_trade,
    },
    users::handler::{
        create_wallet_challenge, join_waitlist, list_trading_providers, update_trading_provider,
        update_wallet,
    },
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

#[utoipa::path(
    get,
    path = "/api/v1/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Application and database are ready", body = ApiResponse<String>),
        (status = 503, description = "Application is not ready", body = ErrorResponse)
    )
)]
async fn readiness_check(State(state): State<AppState>) -> Response {
    let backend = state.db.get_database_backend();
    let result = state
        .db
        .query_one_raw(Statement::from_string(backend, "SELECT 1"))
        .await;

    match result {
        Ok(_) => ok("Application is ready", "Database connection is healthy").into_response(),
        Err(detail) => {
            error!(error = %detail, "readiness database check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "success": false,
                    "message": "Application is not ready"
                })),
            )
                .into_response()
        }
    }
}

fn auth_router(config: &AppConfig) -> Router<AppState> {
    Router::new()
        .route("/signup", post(signup))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/logout-all", post(logout_all))
        .route("/verify-email", post(verify_email))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        .route("/challenge", post(create_challenge))
        .route("/verify", post(verify_challenge))
        .layer(middleware::from_fn_with_state(
            RateLimiter::per_minute(config.auth_rate_limit_per_minute),
            rate_limit::enforce,
        ))
}

fn external_proxy_router(config: &AppConfig) -> Router<AppState> {
    Router::new()
        .route("/markets", get(fetch_markets))
        .route("/markets/{market_id}", get(fetch_market))
        .route("/order-books/{token_id}", get(fetch_order_book))
        .route("/venue-chain", get(fetch_venue_chain))
        .layer(middleware::from_fn_with_state(
            RateLimiter::per_minute(config.external_rate_limit_per_minute),
            rate_limit::enforce,
        ))
}

fn api_v1_router(config: &AppConfig) -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .nest("/auth", auth_router(config))
        .route("/auth/me", get(current_user))
        .route("/venue-connections/polymarket", post(connect_polymarket))
        .route("/analytics/overview", get(analytics_overview))
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
        .route("/markets/favorites", get(list_market_favorites))
        .route(
            "/markets/{market_id}/favorite",
            get(get_market_favorite_status)
                .put(favorite_market)
                .delete(unfavorite_market),
        )
        .route(
            "/markets/{market_id}/comments",
            get(list_market_comments).post(create_market_comment),
        )
        .route(
            "/markets/{market_id}/comments/stream",
            get(stream_market_comments),
        )
        .nest("/polymarket", external_proxy_router(config))
        .route("/trades", get(list_trades))
        .route("/trades/preflight", post(create_trade_intent))
        .route("/trades/cancel-multiple", post(cancel_multiple_trades))
        .route("/trades/cancel-all", post(cancel_all_trades))
        .route("/trades/cancel-market", post(cancel_market_trades))
        .route("/trades/{trade_id}", get(get_trade))
        .route("/trades/{trade_id}/submit", post(submit_signed_trade))
        .route("/trades/{trade_id}/reconcile", post(reconcile_trade))
        .route("/trades/{trade_id}/cancel", post(cancel_trade))
        .route("/trading-providers", get(list_trading_providers))
        .route("/users/settings/email", patch(update_email))
        .route("/users/settings/password", patch(update_password))
        .route("/users/settings/username", patch(update_username))
        .route("/users/trading-provider", patch(update_trading_provider))
        .route("/users/wallet/challenge", post(create_wallet_challenge))
        .route("/users/wallet", patch(update_wallet))
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

fn is_allowed_origin(origin: &HeaderValue, production: bool, allowed_origins: &[String]) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let normalized = origin.trim_end_matches('/');
    let parsed = Url::parse(origin).ok();
    let localhost = parsed.as_ref().is_some_and(|url| {
        url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        })
    });

    if production && localhost {
        return false;
    }

    allowed_origins.iter().any(|allowed| allowed == normalized) || (!production && localhost)
}

fn cors_layer(config: &AppConfig) -> CorsLayer {
    let production = config.is_production();
    let allowed_origins = Arc::new(config.cors_allowed_origins.clone());

    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _request_parts| {
            is_allowed_origin(origin, production, &allowed_origins)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("idempotency-key"),
        ])
}

pub fn create_app(state: AppState) -> Router {
    let config = state.config.clone();
    let request_id_header = HeaderName::from_static("x-request-id");
    let trace_request_id_header = request_id_header.clone();
    let mut app = Router::new()
        .route("/", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/mcp", post(handle_mcp))
        .nest("/api/v1", api_v1_router(&config));

    if config.swagger_enabled {
        app = app.merge(swagger_ui());
    }

    app.layer(middleware::from_fn_with_state(
        RateLimiter::per_minute(config.public_rate_limit_per_minute),
        rate_limit::enforce,
    ))
    .layer(DefaultBodyLimit::max(config.request_body_limit_bytes))
    .layer(ConcurrencyLimitLayer::new(config.concurrency_limit))
    .layer(
        TraceLayer::new_for_http()
            .make_span_with(move |request: &Request<Body>| {
                let request_id = request
                    .headers()
                    .get(&trace_request_id_header)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("unknown");
                tracing::info_span!(
                    "http_request",
                    request_id,
                    method = %request.method(),
                    uri = %request.uri()
                )
            })
            .on_request(DefaultOnRequest::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    )
    .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
    .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
    .layer(cors_layer(&config))
    .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::{api_v1_router, is_allowed_origin};
    use axum::{
        Router,
        body::Body,
        http::{HeaderValue, Method, Request, StatusCode, header::CONTENT_TYPE},
    };
    use sea_orm::DatabaseConnection;
    use tower::ServiceExt;

    use crate::{
        analytics::service::AnalyticsService,
        app::state::AppState,
        auth::service::AuthService,
        automations::service::AutomationService,
        config::AppConfig,
        markets::{
            comments::service::MarketCommentService, favorites::service::MarketFavoriteService,
        },
        notifications::service::NotificationService,
        polymarket::client::PolymarketClient,
        trades::service::TradeService,
        users::service::UserService,
    };

    fn origins() -> Vec<String> {
        vec!["https://www.uptions.xyz".to_owned()]
    }

    fn test_config() -> AppConfig {
        AppConfig {
            server_address: "127.0.0.1:0".to_owned(),
            database_url: "postgres://unused".to_owned(),
            credential_encryption_key:
                "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            app_base_url: "http://localhost:5173".to_owned(),
            polymarket_clob_host: "https://clob.polymarket.com".to_owned(),
            polymarket_gamma_host: "https://gamma-api.polymarket.com".to_owned(),
            polymarket_user_ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws/user".to_owned(),
            environment: "test".to_owned(),
            swagger_enabled: false,
            cors_allowed_origins: origins(),
            request_body_limit_bytes: 1_048_576,
            concurrency_limit: 256,
            public_rate_limit_per_minute: 120,
            auth_rate_limit_per_minute: 10,
            external_rate_limit_per_minute: 60,
        }
    }

    fn test_api_v1_router() -> Router {
        let config = test_config();
        let db = DatabaseConnection::default();
        let notification_service = NotificationService::new();
        let polymarket_client = PolymarketClient::new(&config);
        let state = AppState {
            analytics_service: AnalyticsService::new(db.clone()),
            auth_service: AuthService::new(
                db.clone(),
                config.credential_encryption_key.clone(),
                config.app_base_url.clone(),
            ),
            automation_service: AutomationService::new(db.clone(), notification_service.clone()),
            config: config.clone(),
            db: db.clone(),
            market_comment_service: MarketCommentService::new(db.clone()),
            market_favorite_service: MarketFavoriteService::new(db.clone()),
            notification_service,
            polymarket_client: polymarket_client.clone(),
            trade_service: TradeService::new(
                db.clone(),
                polymarket_client,
                config.credential_encryption_key.clone(),
            ),
            user_service: UserService::new(db),
        };

        api_v1_router(&config).with_state(state)
    }

    #[tokio::test]
    async fn market_favorite_and_comment_routes_are_registered_and_protected() {
        let app = test_api_v1_router();
        let routes = [
            (Method::GET, "/markets/favorites"),
            (Method::GET, "/markets/market-123/favorite"),
            (Method::PUT, "/markets/market-123/favorite"),
            (Method::DELETE, "/markets/market-123/favorite"),
            (Method::GET, "/markets/market-123/comments"),
            (Method::POST, "/markets/market-123/comments"),
            (Method::GET, "/markets/market-123/comments/stream"),
        ];

        for (method, path) in routes {
            let request = Request::builder()
                .method(method.clone())
                .uri(path)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"body":"test comment"}"#))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();

            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {path} should be registered and require authentication"
            );
        }
    }

    #[tokio::test]
    async fn username_settings_route_is_registered_and_protected() {
        let request = Request::builder()
            .method(Method::PATCH)
            .uri("/users/settings/username")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"username":"alice_123"}"#))
            .unwrap();
        let response = test_api_v1_router().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn allows_configured_production_origin() {
        assert!(is_allowed_origin(
            &HeaderValue::from_static("https://www.uptions.xyz"),
            true,
            &origins(),
        ));
    }

    #[test]
    fn rejects_localhost_in_production_even_when_configured() {
        let origins = vec!["http://localhost:5173".to_owned()];

        assert!(!is_allowed_origin(
            &HeaderValue::from_static("http://localhost:5173"),
            true,
            &origins,
        ));
    }

    #[test]
    fn allows_localhost_in_development() {
        assert!(is_allowed_origin(
            &HeaderValue::from_static("http://localhost:5173"),
            false,
            &origins(),
        ));
        assert!(is_allowed_origin(
            &HeaderValue::from_static("http://127.0.0.1:3000"),
            false,
            &origins(),
        ));
    }

    #[test]
    fn rejects_other_origins() {
        assert!(!is_allowed_origin(
            &HeaderValue::from_static("https://uptions.xyz"),
            true,
            &origins(),
        ));
        assert!(!is_allowed_origin(
            &HeaderValue::from_static("https://example.com"),
            true,
            &origins(),
        ));
    }
}
