use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignupRequest {
    #[schema(example = "user@uptions.com")]
    pub email: String,
    #[schema(example = "correct horse battery staple")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "user@uptions.com")]
    pub email: String,
    #[schema(example = "correct horse battery staple")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyEmailRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForgotPasswordRequest {
    #[schema(example = "user@uptions.com")]
    pub email: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub token: String,
    #[schema(example = "correct horse battery staple")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateChallengeRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateChallengeResponse {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub nonce: String,
    #[schema(
        example = "Sign in to Uptions\nAddress: 0x1234567890abcdef1234567890abcdef12345678\nNonce: 550e8400-e29b-41d4-a716-446655440000"
    )]
    pub message: String,
    #[schema(example = 1760000000)]
    pub expires_at: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyChallengeRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    #[schema(
        example = "0x5f2c9c0d93b1b3fddc55c4f98ccf5281af2c0612fd4f2cfd2c7d4dd4f3838f620dcf54e02db91f7df0ec6ee25b9e6f74fd839cc13a5d08d64f6b3db2de4d6c881b"
    )]
    pub signature: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthUserResponse {
    #[schema(example = "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d")]
    pub id: String,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub primary_wallet_address: Option<String>,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: Option<String>,
    #[schema(example = "user@uptions.com")]
    pub email: Option<String>,
    #[schema(example = true)]
    pub email_verified: bool,
    pub venue_connections: Vec<VenueConnectionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VerifyChallengeResponse {
    #[schema(example = "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d")]
    pub access_token: String,
    #[schema(example = "Bearer")]
    pub token_type: String,
    pub user: AuthUserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthSessionResponse {
    #[schema(example = "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d")]
    pub access_token: String,
    #[schema(example = "Bearer")]
    pub token_type: String,
    pub expires_at: i64,
    pub user: AuthUserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VenueConnectionResponse {
    #[schema(example = "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d")]
    pub id: String,
    #[schema(example = "polymarket")]
    pub venue: String,
    #[schema(example = "api_key")]
    pub auth_type: String,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub account_identifier: String,
    #[schema(example = true)]
    pub enabled: bool,
    pub limits: Value,
    pub permissions: Value,
    #[schema(example = "active")]
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConnectPolymarketRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub account_identifier: Option<String>,
    #[schema(example = "3e8f4f1a-3be4-43ef-a9b3-df6d83cc66cc")]
    pub api_key: String,
    #[schema(example = "base64-secret-value")]
    pub secret: String,
    #[schema(example = "polymarket-passphrase")]
    pub passphrase: String,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub funder: Option<String>,
    #[schema(example = 3)]
    pub signature_type: Option<i32>,
    pub limits: Option<Value>,
    pub permissions: Option<Value>,
}
