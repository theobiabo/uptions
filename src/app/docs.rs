use utoipa::{
    Modify, OpenApi,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    analytics::dto::{
        AnalyticsCounts, AnalyticsOverviewResponse, DailyActivity, PerformanceAvailability,
        PnlAvailability, StatusCount, WorkflowActivity,
    },
    auth::dto::{
        AccountWarningResponse, AuthSessionResponse, AuthUserResponse, CreateChallengeRequest,
        CreateChallengeResponse, ForgotPasswordRequest, LoginRequest, LogoutResponse,
        ResetPasswordRequest, SettingsUpdateResponse, SignupRequest, UpdateEmailRequest,
        UpdatePasswordRequest, UpdateUsernameRequest, VenueConnectionResponse,
        VerifyChallengeRequest, VerifyChallengeResponse, VerifyEmailRequest,
        WalletChallengeRequest, WalletChallengeResponse,
    },
    automations::dto::{
        AutomationAlertResponse, AutomationMarketPayload, AutomationResponse, AutomationStatus,
        AutomationStepKind, ClearAlertsResponse, MarkAlertsReadResponse, PublishAutomationRequest,
        TestRunAutomationRequest, TestRunAutomationResponse, UpdateAutomationStatusRequest,
        WorkflowActionType, WorkflowConnectionPayload, WorkflowPayload, WorkflowStepPayload,
    },
    error::ErrorResponse,
    markets::{
        comments::dto::{
            CreateMarketCommentRequest, MarketCommentAuthorResponse, MarketCommentResponse,
            MarketCommentStreamEvent, MarketCommentsPageResponse, MarketCommentsQuery,
        },
        favorites::dto::{
            MarketFavoriteStatusResponse, MarketFavoritesPageResponse, MarketFavoritesQuery,
        },
        types::{
            MarketListQuery, MarketOutcomeResponse, MarketPageResponse, MarketResponse,
            MarketTradingMetadata, OrderBookLevel, OrderBookResponse,
        },
    },
    mcp::dto::{
        McpApprovalDecisionResponse, McpApprovalResponse, McpJsonRpcRequest, McpJsonRpcResponse,
    },
    notifications::dto::AutomationAlertStreamEvent,
    providers::{
        polymarket::{
            credentials::ConnectPolymarketRequest,
            dto::{PolymarketExecutionType, PolymarketTokenMetadataResponse},
        },
        types::{Chain, ChainId, ProviderCapability, ProviderId, ProviderResponse},
    },
    response::ApiResponse,
    trades::dto::{
        CancelMarketTradesRequest, CancelMultipleTradesRequest, CancelTradesResponse,
        CreateTradeIntentRequest, CreateTradeIntentResponse, ReconcileTradeResponse,
        SubmitSignedTradeRequest, SubmitSignedTradeResponse, TradeIntentResponse, TradeOrderType,
        TradeSide,
    },
    users::handler::{
        UpdateTradingProviderRequest, UpdateWalletRequest, UserTradingProviderResponse,
        UserWalletResponse, WaitlistResponse, WaitlistUser,
    },
};

#[derive(OpenApi)]
#[openapi(
    paths(
        super::health_check,
        super::readiness_check,
        crate::analytics::handlers::analytics_overview,
        crate::auth::handlers::signup,
        crate::auth::handlers::login,
        crate::auth::handlers::logout,
        crate::auth::handlers::logout_all,
        crate::auth::handlers::verify_email,
        crate::auth::handlers::forgot_password,
        crate::auth::handlers::reset_password,
        crate::auth::handlers::create_challenge,
        crate::auth::handlers::verify_challenge,
        crate::auth::handlers::current_user,
        crate::auth::handlers::update_email,
        crate::auth::handlers::update_password,
        crate::auth::handlers::update_username,

        crate::auth::handlers::connect_provider,
        crate::automations::handlers::list_automations,
        crate::automations::handlers::publish_automation,
        crate::automations::handlers::update_automation,
        crate::automations::handlers::update_automation_status,
        crate::automations::handlers::delete_automation,
        crate::automations::handlers::test_run_automation,
        crate::automations::handlers::list_alerts,
        crate::automations::handlers::clear_alerts,
        crate::automations::handlers::mark_alerts_read,
        crate::automations::handlers::mark_alert_read,

        crate::markets::comments::handlers::list_provider_market_comments,
        crate::markets::comments::handlers::create_provider_market_comment,
        crate::markets::comments::handlers::stream_provider_market_comments,

        crate::markets::favorites::handlers::favorite_provider_market,
        crate::markets::favorites::handlers::unfavorite_provider_market,
        crate::markets::favorites::handlers::get_provider_market_favorite_status,
        crate::markets::favorites::handlers::list_provider_market_favorites,
        crate::mcp::handlers::handle_mcp,
        crate::mcp::handlers::list_mcp_approvals,
        crate::mcp::handlers::get_mcp_approval,
        crate::mcp::handlers::approve_mcp_approval,
        crate::mcp::handlers::reject_mcp_approval,
        crate::notifications::handlers::stream_alerts,

        crate::providers::handlers::list_providers,
        crate::providers::handlers::get_provider,
        crate::providers::handlers::fetch_markets,
        crate::providers::handlers::fetch_market,
        crate::providers::handlers::fetch_order_book,
        crate::trades::handlers::list_trades,
        crate::trades::handlers::get_trade,
        crate::trades::handlers::create_trade_intent,
        crate::trades::handlers::submit_signed_trade,
        crate::trades::handlers::reconcile_trade,
        crate::trades::handlers::cancel_trade,
        crate::trades::handlers::cancel_multiple_trades,
        crate::trades::handlers::cancel_all_trades,
        crate::trades::handlers::cancel_market_trades,

        crate::users::handler::update_trading_provider,
        crate::users::handler::create_wallet_challenge,
        crate::users::handler::update_wallet,
        crate::users::handler::join_waitlist
    ),
    components(
        schemas(
            AnalyticsCounts,
            AnalyticsOverviewResponse,
            DailyActivity,
            PerformanceAvailability,
            PnlAvailability,
            StatusCount,
            WorkflowActivity,
            ApiResponse<AnalyticsOverviewResponse>,
            AuthUserResponse,
            AuthSessionResponse,
            AccountWarningResponse,
            AutomationAlertResponse,
            AutomationResponse,
            AutomationMarketPayload,
            AutomationAlertStreamEvent,
            AutomationStatus,
            AutomationStepKind,
            ApiResponse<AuthUserResponse>,
            ApiResponse<AuthSessionResponse>,
            ApiResponse<AutomationResponse>,
            ApiResponse<TestRunAutomationResponse>,
            ApiResponse<Vec<AutomationAlertResponse>>,
            ApiResponse<Vec<AutomationResponse>>,
            ApiResponse<MarkAlertsReadResponse>,
            ApiResponse<ClearAlertsResponse>,
            ApiResponse<McpApprovalResponse>,
            ApiResponse<McpApprovalDecisionResponse>,
            ApiResponse<Vec<McpApprovalResponse>>,
            ApiResponse<CreateChallengeResponse>,
            ApiResponse<String>,
            ApiResponse<VenueConnectionResponse>,
            ApiResponse<VerifyChallengeResponse>,
            ApiResponse<WaitlistResponse>,
            ApiResponse<MarketPageResponse>,
            ApiResponse<MarketResponse>,
            ApiResponse<OrderBookResponse>,
            ApiResponse<ProviderResponse>,
            ApiResponse<Vec<ProviderResponse>>,
            ApiResponse<UserTradingProviderResponse>,
            ApiResponse<UserWalletResponse>,
            ApiResponse<SettingsUpdateResponse>,
            ApiResponse<LogoutResponse>,
            ApiResponse<WalletChallengeResponse>,
            ApiResponse<Vec<TradeIntentResponse>>,
            ApiResponse<TradeIntentResponse>,
            ApiResponse<CreateTradeIntentResponse>,
            ApiResponse<SubmitSignedTradeResponse>,
            ApiResponse<ReconcileTradeResponse>,
            ApiResponse<CancelTradesResponse>,
            ConnectPolymarketRequest,
            CreateChallengeRequest,
            CreateChallengeResponse,
            ErrorResponse,
            ForgotPasswordRequest,
            LoginRequest,
            LogoutResponse,
            MarketListQuery,
            MarkAlertsReadResponse,
            ClearAlertsResponse,
            MarketCommentAuthorResponse,
            MarketCommentResponse,
            MarketCommentStreamEvent,
            MarketCommentsPageResponse,
            MarketCommentsQuery,
            CreateMarketCommentRequest,
            ApiResponse<MarketCommentResponse>,
            ApiResponse<MarketCommentsPageResponse>,
            MarketFavoriteStatusResponse,
            MarketFavoritesPageResponse,
            MarketFavoritesQuery,
            ApiResponse<MarketFavoriteStatusResponse>,
            ApiResponse<MarketFavoritesPageResponse>,
            McpApprovalResponse,
            McpApprovalDecisionResponse,
            McpJsonRpcRequest,
            McpJsonRpcResponse,
            MarketOutcomeResponse,
            MarketPageResponse,
            MarketResponse,
            MarketTradingMetadata,
            OrderBookLevel,
            OrderBookResponse,
            PolymarketTokenMetadataResponse,
            CancelMarketTradesRequest,
            CancelMultipleTradesRequest,
            CancelTradesResponse,
            CreateTradeIntentRequest,
            CreateTradeIntentResponse,
            PolymarketExecutionType,
            ReconcileTradeResponse,
            SubmitSignedTradeRequest,
            SubmitSignedTradeResponse,
            TradeIntentResponse,
            TradeOrderType,
            TradeSide,
            PublishAutomationRequest,
            ResetPasswordRequest,
            SettingsUpdateResponse,
            SignupRequest,
            UpdateEmailRequest,
            UpdatePasswordRequest,
            UpdateUsernameRequest,
            TestRunAutomationRequest,
            TestRunAutomationResponse,

            UpdateAutomationStatusRequest,
            UpdateTradingProviderRequest,
            UpdateWalletRequest,
            WalletChallengeRequest,
            WalletChallengeResponse,
            WorkflowActionType,
            WorkflowConnectionPayload,
            WorkflowPayload,
            WorkflowStepPayload,
            UserTradingProviderResponse,
            UserWalletResponse,
            Chain,
            ChainId,
            ProviderCapability,
            ProviderId,
            ProviderResponse,
            VenueConnectionResponse,
            VerifyChallengeRequest,
            VerifyChallengeResponse,
            VerifyEmailRequest,
            WaitlistResponse,
            WaitlistUser
        )
    ),
    modifiers(&SecurityAddon),
    info(
        title = "Uptions Backend API",
        version = "1.0.0",
        description = "Versioned V1 backend endpoints for Uptions identity, venue connections, market discovery, and automation workflows."
    )
)]
struct ApiDoc;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi())
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use utoipa::OpenApi;

    use super::ApiDoc;

    #[test]
    fn auth_user_openapi_documents_structured_account_warnings() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let auth_user = &document["components"]["schemas"]["AuthUserResponse"];
        let warnings = &auth_user["properties"]["account_warnings"];
        let warning = &document["components"]["schemas"]["AccountWarningResponse"];

        assert_eq!(warnings["type"], "array");
        assert_eq!(
            warnings["items"]["$ref"],
            "#/components/schemas/AccountWarningResponse"
        );
        for field in [
            "code",
            "severity",
            "title",
            "message",
            "action_label",
            "action_href",
        ] {
            assert!(warning["properties"][field].is_object());
            assert!(
                warning["required"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|required| required == field)
            );
        }
    }

    #[test]
    fn only_canonical_provider_paths_are_documented() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for path in [
            "/api/v1/providers",
            "/api/v1/providers/{provider}",
            "/api/v1/providers/{provider}/markets",
            "/api/v1/providers/{provider}/markets/{market_id}",
            "/api/v1/providers/{provider}/markets/{market_id}/order-book",
            "/api/v1/providers/{provider}/connection",
            "/api/v1/providers/{provider}/markets/favorites",
            "/api/v1/providers/{provider}/markets/{market_id}/comments",
            "/api/v1/providers/{provider}/markets/{market_id}/comments/stream",
            "/api/v1/providers/{provider}/markets/{market_id}/favorite",
        ] {
            assert!(document["paths"][path].is_object(), "missing {path}");
        }

        for path in [
            "/api/v1/polymarket/markets",
            "/api/v1/polymarket/markets/{market_id}",
            "/api/v1/polymarket/order-books/{token_id}",
            "/api/v1/polymarket/venue-chain",
            "/api/v1/venue-connections/polymarket",
            "/api/v1/markets/favorites",
            "/api/v1/markets/{market_id}/favorite",
            "/api/v1/markets/{market_id}/comments",
            "/api/v1/markets/{market_id}/comments/stream",
            "/api/v1/trading-providers",
            "/api/v1/providers/{provider}/order-books/{instrument_id}",
            "/api/v1/providers/{provider}/venue-connection",
            "/api/v1/providers/{provider}/venue-chain",
        ] {
            assert!(document["paths"].get(path).is_none(), "stale {path}");
        }

        let tags = document["paths"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(Value::as_object)
            .flat_map(|operations| operations.values())
            .filter_map(|operation| operation.get("tags"))
            .filter_map(Value::as_array)
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert!(!tags.contains(&"Polymarket Compatibility"));
    }

    #[test]
    fn market_routes_use_concrete_normalized_schemas() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let markets = &document["paths"]["/api/v1/providers/{provider}/markets"]["get"]["responses"]
            ["200"]["content"]["application/json"]["schema"];
        let market = &document["paths"]["/api/v1/providers/{provider}/markets/{market_id}"]["get"]
            ["responses"]["200"]["content"]["application/json"]["schema"];
        let order_book = &document["paths"]["/api/v1/providers/{provider}/markets/{market_id}/order-book"]
            ["get"]["responses"]["200"]["content"]["application/json"]["schema"];

        assert_eq!(
            markets["$ref"],
            "#/components/schemas/ApiResponse_MarketPageResponse"
        );
        assert_eq!(
            market["$ref"],
            "#/components/schemas/ApiResponse_MarketResponse"
        );
        assert_eq!(
            order_book["$ref"],
            "#/components/schemas/ApiResponse_OrderBookResponse"
        );
    }

    #[test]
    fn username_settings_path_and_schema_are_registered() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let patch = &document["paths"]["/api/v1/users/settings/username"]["patch"];

        assert!(patch.is_object());
        assert_eq!(
            patch["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            Value::String("#/components/schemas/UpdateUsernameRequest".to_owned())
        );
        assert!(document["components"]["schemas"]["UpdateUsernameRequest"].is_object());
        assert!(patch["responses"]["200"].is_object());
        assert!(patch["responses"]["409"].is_object());
    }
}
