use utoipa::{
    Modify, OpenApi,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth::dto::{
        AuthSessionResponse, AuthUserResponse, ConnectPolymarketRequest, CreateChallengeRequest,
        CreateChallengeResponse, ForgotPasswordRequest, LoginRequest, ResetPasswordRequest,
        SignupRequest, VenueConnectionResponse, VerifyChallengeRequest, VerifyChallengeResponse,
        VerifyEmailRequest,
    },
    automations::dto::{
        AutomationAlertResponse, AutomationMarketPayload, AutomationProvider, AutomationResponse,
        AutomationStatus, AutomationStepKind, MarkAlertsReadResponse, PublishAutomationRequest,
        TestRunAutomationRequest, TestRunAutomationResponse, UpdateAutomationStatusRequest,
        WorkflowActionType, WorkflowConnectionPayload, WorkflowPayload, WorkflowStepPayload,
    },
    error::ErrorResponse,
    mcp::dto::{McpJsonRpcRequest, McpJsonRpcResponse},
    notifications::dto::AutomationAlertStreamEvent,
    polymarket::dto::MarketsQuery,
    response::ApiResponse,
    users::handler::{WaitlistResponse, WaitlistUser},
};

#[derive(OpenApi)]
#[openapi(
    paths(
        super::health_check,
        crate::auth::handlers::signup,
        crate::auth::handlers::login,
        crate::auth::handlers::verify_email,
        crate::auth::handlers::forgot_password,
        crate::auth::handlers::reset_password,
        crate::auth::handlers::create_challenge,
        crate::auth::handlers::verify_challenge,
        crate::auth::handlers::current_user,
        crate::auth::handlers::connect_polymarket,
        crate::automations::handlers::list_automations,
        crate::automations::handlers::publish_automation,
        crate::automations::handlers::update_automation,
        crate::automations::handlers::update_automation_status,
        crate::automations::handlers::delete_automation,
        crate::automations::handlers::test_run_automation,
        crate::automations::handlers::list_alerts,
        crate::automations::handlers::mark_alerts_read,
        crate::automations::handlers::mark_alert_read,
        crate::mcp::handlers::handle_mcp,
        crate::notifications::handlers::stream_alerts,
        crate::polymarket::handlers::fetch_markets,
        crate::polymarket::handlers::fetch_market,
        crate::users::handler::join_waitlist
    ),
    components(
        schemas(
            AuthUserResponse,
            AuthSessionResponse,
            AutomationAlertResponse,
            AutomationResponse,
            AutomationMarketPayload,
            AutomationProvider,
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
            ApiResponse<CreateChallengeResponse>,
            ApiResponse<String>,
            ApiResponse<VenueConnectionResponse>,
            ApiResponse<VerifyChallengeResponse>,
            ApiResponse<WaitlistResponse>,
            ConnectPolymarketRequest,
            CreateChallengeRequest,
            CreateChallengeResponse,
            ErrorResponse,
            ForgotPasswordRequest,
            LoginRequest,
            MarketsQuery,
            MarkAlertsReadResponse,
            McpJsonRpcRequest,
            McpJsonRpcResponse,
            PublishAutomationRequest,
            ResetPasswordRequest,
            SignupRequest,
            TestRunAutomationRequest,
            TestRunAutomationResponse,
            UpdateAutomationStatusRequest,
            WorkflowActionType,
            WorkflowConnectionPayload,
            WorkflowPayload,
            WorkflowStepPayload,
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
