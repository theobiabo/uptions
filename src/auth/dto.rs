use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::providers::{
    polymarket::connection::EligibilityFailure,
    types::{Chain, ChainId, ProviderId},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct SignupRequest {
    #[schema(example = "user@uptions.com")]
    pub email: String,
    #[schema(example = "correct horse battery staple")]
    pub password: String,
    #[schema(example = "uptions_user")]
    pub username: String,
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
pub struct UpdateEmailRequest {
    #[schema(example = "user@uptions.com")]
    pub email: String,
    #[schema(example = "correct horse battery staple")]
    pub current_password: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePasswordRequest {
    #[schema(example = "correct horse battery staple")]
    pub current_password: String,
    #[schema(example = "new correct horse battery staple")]
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUsernameRequest {
    #[schema(example = "uptions_user")]
    pub username: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SettingsUpdateResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogoutResponse {
    pub revoked_sessions: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct WalletChallengeRequest {
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    pub provider: ProviderId,
    pub chain: Chain,
    #[schema(example = 137)]
    pub chain_id: ChainId,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WalletChallengeResponse {
    #[schema(example = "associate_wallet")]
    pub purpose: String,
    pub provider: ProviderId,
    pub chain: Chain,
    #[schema(example = 137)]
    pub chain_id: ChainId,
    #[schema(example = "0x1234567890abcdef1234567890abcdef12345678")]
    pub wallet_address: String,
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub nonce: String,
    pub message: String,
    #[schema(example = 1760000000)]
    pub expires_at: u64,
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
    #[schema(example = "uptions_user")]
    pub username: Option<String>,
    #[schema(example = true)]
    pub email_verified: bool,
    #[schema(example = true)]
    pub password_configured: bool,
    pub preferred_trading_provider: ProviderId,
    pub venue_connections: Vec<VenueConnectionResponse>,
    pub account_warnings: Vec<AccountWarningResponse>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
pub struct AccountWarningResponse {
    #[schema(example = "polymarket_wallet_mismatch")]
    pub code: String,
    #[schema(example = "warning")]
    pub severity: String,
    #[schema(example = "Reconnect Polymarket")]
    pub title: String,
    #[schema(example = "Your Polymarket connection no longer matches your connected wallet.")]
    pub message: String,
    #[schema(example = "Reconnect Polymarket")]
    pub action_label: String,
    #[schema(example = "/settings#trading")]
    pub action_href: String,
}

pub(crate) fn account_warning_for_connection(
    venue: &str,
    enabled: bool,
    status: &str,
) -> Option<AccountWarningResponse> {
    if ProviderId::from_storage(venue) != Some(ProviderId::Polymarket) || !enabled {
        return None;
    }

    let failure = EligibilityFailure::from_status(status)?;
    let (title, message, action_label) = match failure {
        EligibilityFailure::WalletMissing => (
            "Connect a wallet",
            "Your Polymarket connection needs a connected wallet before private updates can resume.",
            "Connect wallet",
        ),
        EligibilityFailure::WalletMismatch => (
            "Reconnect Polymarket",
            "Your Polymarket connection no longer matches your connected wallet.",
            "Reconnect Polymarket",
        ),
        EligibilityFailure::UnsupportedAccount => (
            "Use a supported Polymarket account",
            "Reconnect Polymarket with an EOA account whose signer, account, and funder match.",
            "Reconnect with EOA",
        ),
        EligibilityFailure::InvalidCredentials => (
            "Update Polymarket credentials",
            "Your saved Polymarket credentials are invalid or incomplete.",
            "Update credentials",
        ),
    };

    Some(AccountWarningResponse {
        code: failure.code().to_owned(),
        severity: "warning".to_owned(),
        title: title.to_owned(),
        message: message.to_owned(),
        action_label: action_label.to_owned(),
        action_href: "/settings#trading".to_owned(),
    })
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
    pub provider: ProviderId,
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

#[cfg(test)]
mod tests {
    use crate::providers::polymarket::connection::{
        INVALID_CREDENTIALS_STATUS, UNSUPPORTED_ACCOUNT_STATUS, WALLET_MISMATCH_STATUS,
        WALLET_MISSING_STATUS,
    };

    use super::account_warning_for_connection;

    #[test]
    fn account_warnings_map_action_required_connection_state() {
        let cases = [
            (
                WALLET_MISSING_STATUS,
                "polymarket_wallet_missing",
                "Connect wallet",
            ),
            (
                WALLET_MISMATCH_STATUS,
                "polymarket_wallet_mismatch",
                "Reconnect Polymarket",
            ),
            (
                UNSUPPORTED_ACCOUNT_STATUS,
                "polymarket_unsupported_account",
                "Reconnect with EOA",
            ),
            (
                INVALID_CREDENTIALS_STATUS,
                "polymarket_invalid_credentials",
                "Update credentials",
            ),
        ];

        for (status, code, action_label) in cases {
            let warning = account_warning_for_connection("polymarket", true, status).unwrap();
            assert_eq!(warning.code, code);
            assert_eq!(warning.severity, "warning");
            assert_eq!(warning.action_label, action_label);
            assert_eq!(warning.action_href, "/settings#trading");
            assert!(!warning.title.is_empty());
            assert!(!warning.message.is_empty());
        }
    }

    #[test]
    fn account_warnings_ignore_active_disabled_and_other_venue_connections() {
        assert!(account_warning_for_connection("polymarket", true, "active").is_none());
        assert!(
            account_warning_for_connection("polymarket", false, WALLET_MISMATCH_STATUS).is_none()
        );
        assert!(account_warning_for_connection("kalshi", true, WALLET_MISMATCH_STATUS).is_none());
    }

    #[test]
    fn account_warning_dto_contains_no_connection_or_credential_details() {
        let warning =
            account_warning_for_connection("polymarket", true, INVALID_CREDENTIALS_STATUS).unwrap();
        let value = serde_json::to_value(warning).unwrap();

        assert_eq!(value.as_object().unwrap().len(), 6);
        assert!(value.get("credentials").is_none());
        assert!(value.get("account_identifier").is_none());
        assert!(value.get("connection_id").is_none());
    }
}
