use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateChallengeRequest {
    pub wallet_address: String,
}

#[derive(Debug, Serialize)]
pub struct CreateChallengeResponse {
    pub wallet_address: String,
    pub nonce: String,
    pub message: String,
    pub expires_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct VerifyChallengeRequest {
    pub wallet_address: String,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct AuthUserResponse {
    pub wallet_address: String,
    pub polymarket_linked: bool,
}

#[derive(Debug, Serialize)]
pub struct VerifyChallengeResponse {
    pub access_token: String,
    pub token_type: String,
    pub user: AuthUserResponse,
}
