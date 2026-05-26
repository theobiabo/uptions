use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    #[schema(example = false)]
    pub polymarket_linked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VerifyChallengeResponse {
    #[schema(example = "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d")]
    pub access_token: String,
    #[schema(example = "Bearer")]
    pub token_type: String,
    pub user: AuthUserResponse,
}
