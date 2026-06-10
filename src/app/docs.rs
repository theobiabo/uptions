use axum::{
    Json,
    response::{Html, IntoResponse},
};
use utoipa::{
    Modify, OpenApi,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};

use crate::{
    auth::dto::{
        AuthSessionResponse, AuthUserResponse, ConnectPolymarketRequest, CreateChallengeRequest,
        CreateChallengeResponse, ForgotPasswordRequest, LoginRequest, ResetPasswordRequest,
        SignupRequest, VenueConnectionResponse, VerifyChallengeRequest, VerifyChallengeResponse,
        VerifyEmailRequest,
    },
    error::ErrorResponse,
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
        crate::polymarket::handlers::fetch_markets,
        crate::users::handler::join_waitlist
    ),
    components(
        schemas(
            AuthUserResponse,
            AuthSessionResponse,
            ApiResponse<AuthUserResponse>,
            ApiResponse<AuthSessionResponse>,
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
            ResetPasswordRequest,
            SignupRequest,
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
        description = "Versioned V1 backend endpoints for Uptions identity, venue connections, and market discovery."
    )
)]
struct ApiDoc;

pub async fn openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

pub async fn swagger_ui() -> impl IntoResponse {
    Html(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Uptions API Documentation</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.ui = SwaggerUIBundle({
      url: "/docs/openapi.json",
      dom_id: "#swagger-ui"
    });
  </script>
</body>
</html>"##,
    )
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
