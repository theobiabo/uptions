use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

use crate::entities::venue_connection;

pub const POLYMARKET_VENUE: &str = "polymarket";
pub const ACTIVE_STATUS: &str = "active";
pub const WALLET_MISSING_STATUS: &str = "action_required_wallet_missing";
pub const WALLET_MISMATCH_STATUS: &str = "action_required_wallet_mismatch";
pub const UNSUPPORTED_ACCOUNT_STATUS: &str = "action_required_unsupported_eoa";
pub const INVALID_CREDENTIALS_STATUS: &str = "action_required_credentials";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EligibilityFailure {
    WalletMissing,
    WalletMismatch,
    UnsupportedAccount,
    InvalidCredentials,
}

impl EligibilityFailure {
    pub const fn status(self) -> &'static str {
        match self {
            Self::WalletMissing => WALLET_MISSING_STATUS,
            Self::WalletMismatch => WALLET_MISMATCH_STATUS,
            Self::UnsupportedAccount => UNSUPPORTED_ACCOUNT_STATUS,
            Self::InvalidCredentials => INVALID_CREDENTIALS_STATUS,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::WalletMissing => "polymarket_wallet_missing",
            Self::WalletMismatch => "polymarket_wallet_mismatch",
            Self::UnsupportedAccount => "polymarket_unsupported_account",
            Self::InvalidCredentials => "polymarket_invalid_credentials",
        }
    }

    pub const fn log_reason(self) -> &'static str {
        match self {
            Self::WalletMissing => "connected wallet is missing",
            Self::WalletMismatch => "connection account does not match connected wallet",
            Self::UnsupportedAccount => {
                "connection must use a supported EOA account with a matching funder"
            }
            Self::InvalidCredentials => "stored credentials are invalid or incomplete",
        }
    }

    pub fn from_status(status: &str) -> Option<Self> {
        match status {
            WALLET_MISSING_STATUS => Some(Self::WalletMissing),
            WALLET_MISMATCH_STATUS => Some(Self::WalletMismatch),
            UNSUPPORTED_ACCOUNT_STATUS => Some(Self::UnsupportedAccount),
            INVALID_CREDENTIALS_STATUS => Some(Self::InvalidCredentials),
            _ => None,
        }
    }
}

pub fn eligibility_transition(
    current_status: &str,
    failure: EligibilityFailure,
) -> Option<&'static str> {
    (current_status == ACTIVE_STATUS).then(|| failure.status())
}

pub async fn mark_eligibility_failure<C>(
    db: &C,
    connection_id: &str,
    current_status: &str,
    expected_updated_at: sea_orm::prelude::DateTimeWithTimeZone,
    failure: EligibilityFailure,
    now: DateTime<Utc>,
) -> Result<bool, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let Some(status) = eligibility_transition(current_status, failure) else {
        return Ok(false);
    };
    let result = venue_connection::Entity::update_many()
        .set(venue_connection::ActiveModel {
            status: Set(status.to_owned()),
            updated_at: Set(now.into()),
            ..Default::default()
        })
        .filter(venue_connection::Column::Id.eq(connection_id))
        .filter(venue_connection::Column::Status.eq(ACTIVE_STATUS))
        .filter(venue_connection::Column::UpdatedAt.eq(expected_updated_at))
        .exec(db)
        .await?;

    Ok(result.rows_affected == 1)
}

pub async fn reconcile_polymarket_after_wallet_replacement<C>(
    db: &C,
    user_id: &str,
    wallet_address: &str,
    now: DateTime<Utc>,
) -> Result<(u64, u64), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let restored = matching_wallet_restore_update(user_id, wallet_address, now)
        .exec(db)
        .await?;
    let mismatched = wallet_mismatch_update(user_id, wallet_address, now)
        .exec(db)
        .await?;

    Ok((restored.rows_affected, mismatched.rows_affected))
}

fn matching_wallet_restore_update(
    user_id: &str,
    wallet_address: &str,
    now: DateTime<Utc>,
) -> sea_orm::UpdateMany<venue_connection::Entity> {
    venue_connection::Entity::update_many()
        .set(venue_connection::ActiveModel {
            status: Set(ACTIVE_STATUS.to_owned()),
            updated_at: Set(now.into()),
            ..Default::default()
        })
        .filter(venue_connection::Column::UserId.eq(user_id))
        .filter(venue_connection::Column::Venue.eq(POLYMARKET_VENUE))
        .filter(venue_connection::Column::Enabled.eq(true))
        .filter(venue_connection::Column::Status.eq(WALLET_MISMATCH_STATUS))
        .filter(venue_connection::Column::AccountIdentifier.eq(wallet_address))
}

fn wallet_mismatch_update(
    user_id: &str,
    wallet_address: &str,
    now: DateTime<Utc>,
) -> sea_orm::UpdateMany<venue_connection::Entity> {
    venue_connection::Entity::update_many()
        .set(venue_connection::ActiveModel {
            status: Set(WALLET_MISMATCH_STATUS.to_owned()),
            updated_at: Set(now.into()),
            ..Default::default()
        })
        .filter(venue_connection::Column::UserId.eq(user_id))
        .filter(venue_connection::Column::Venue.eq(POLYMARKET_VENUE))
        .filter(venue_connection::Column::Enabled.eq(true))
        .filter(venue_connection::Column::Status.eq(ACTIVE_STATUS))
        .filter(venue_connection::Column::AccountIdentifier.ne(wallet_address))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sea_orm::{DatabaseBackend, QueryTrait};

    use super::{
        ACTIVE_STATUS, EligibilityFailure, INVALID_CREDENTIALS_STATUS, UNSUPPORTED_ACCOUNT_STATUS,
        WALLET_MISMATCH_STATUS, WALLET_MISSING_STATUS, eligibility_transition,
        matching_wallet_restore_update, wallet_mismatch_update,
    };

    #[test]
    fn maps_deterministic_failures_to_stable_action_required_statuses() {
        let cases = [
            (EligibilityFailure::WalletMissing, WALLET_MISSING_STATUS),
            (EligibilityFailure::WalletMismatch, WALLET_MISMATCH_STATUS),
            (
                EligibilityFailure::UnsupportedAccount,
                UNSUPPORTED_ACCOUNT_STATUS,
            ),
            (
                EligibilityFailure::InvalidCredentials,
                INVALID_CREDENTIALS_STATUS,
            ),
        ];

        for (failure, status) in cases {
            assert_eq!(failure.status(), status);
            assert_eq!(EligibilityFailure::from_status(status), Some(failure));
        }
    }

    #[test]
    fn action_required_statuses_fit_storage_column() {
        for status in [
            WALLET_MISSING_STATUS,
            WALLET_MISMATCH_STATUS,
            UNSUPPORTED_ACCOUNT_STATUS,
            INVALID_CREDENTIALS_STATUS,
        ] {
            assert!(status.len() <= 32);
        }
    }

    #[test]
    fn eligibility_transition_is_one_time_and_quiescent() {
        let failure = EligibilityFailure::WalletMismatch;
        let transitioned = eligibility_transition(ACTIVE_STATUS, failure).unwrap();

        assert_eq!(transitioned, WALLET_MISMATCH_STATUS);
        assert_eq!(eligibility_transition(transitioned, failure), None);
    }

    #[test]
    fn wallet_replacement_restore_is_narrowly_scoped_to_matching_wallet_mismatch() {
        let statement = matching_wallet_restore_update(
            "user-1",
            "0x1111111111111111111111111111111111111111",
            Utc::now(),
        )
        .build(DatabaseBackend::Postgres);
        let sql = statement.to_string();

        assert!(sql.contains("\"user_id\" = 'user-1'"));
        assert!(sql.contains("\"venue\" = 'polymarket'"));
        assert!(sql.contains("\"enabled\" = TRUE"));
        assert!(sql.contains("\"status\" = 'action_required_wallet_mismatch'"));
        assert!(
            sql.contains("\"account_identifier\" = '0x1111111111111111111111111111111111111111'")
        );
        assert!(sql.contains("SET \"status\" = 'active'"));
    }

    #[test]
    fn wallet_replacement_update_is_scoped_to_mismatched_active_polymarket_connection() {
        let statement = wallet_mismatch_update(
            "user-1",
            "0x1111111111111111111111111111111111111111",
            Utc::now(),
        )
        .build(DatabaseBackend::Postgres);
        let sql = statement.to_string();

        assert!(sql.contains("\"user_id\" = 'user-1'"));
        assert!(sql.contains("\"venue\" = 'polymarket'"));
        assert!(sql.contains("\"enabled\" = TRUE"));
        assert!(sql.contains("\"status\" = 'active'"));
        assert!(
            sql.contains("\"account_identifier\" <> '0x1111111111111111111111111111111111111111'")
        );
        assert!(sql.contains("SET \"status\" = 'action_required_wallet_mismatch'"));
    }

    #[test]
    fn reconnect_target_status_is_active() {
        assert_eq!(ACTIVE_STATUS, "active");
        assert!(EligibilityFailure::from_status(ACTIVE_STATUS).is_none());
    }
}
