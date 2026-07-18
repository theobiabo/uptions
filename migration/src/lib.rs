pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260527_131801_create_waitlist::Migration),
            Box::new(m20260601_000001_identity_and_venue_connections::Migration),
            Box::new(m20260609_000001_email_auth_and_sessions::Migration),
            Box::new(m20260610_000001_email_verification_and_password_reset::Migration),
            Box::new(m20260707_000001_automations::Migration),
            Box::new(m20260709_000001_automation_alert_reads::Migration),
            Box::new(m20260709_000002_mcp_approval_requests::Migration),
            Box::new(m20260710_000001_automation_runs::Migration),
            Box::new(m20260710_000002_user_trading_provider::Migration),
            Box::new(m20260710_000003_trade_intents::Migration),
            Box::new(m20260713_000001_automation_idempotency_key::Migration),
            Box::new(m20260713_000002_usernames::Migration),
            Box::new(m20260713_000003_automation_observations::Migration),
            Box::new(m20260713_000004_wallet_signature_challenges::Migration),
            Box::new(m20260713_000005_trade_submission_hardening::Migration),
            Box::new(m20260713_000006_polymarket_private_beta_lifecycle::Migration),
            Box::new(m20260718_000001_market_comments::Migration),
            Box::new(m20260718_000002_market_favorites::Migration),
        ]
    }
}
mod m20260527_131801_create_waitlist;
mod m20260601_000001_identity_and_venue_connections;
mod m20260609_000001_email_auth_and_sessions;
mod m20260610_000001_email_verification_and_password_reset;
mod m20260707_000001_automations;
mod m20260709_000001_automation_alert_reads;
mod m20260709_000002_mcp_approval_requests;
mod m20260710_000001_automation_runs;
mod m20260710_000002_user_trading_provider;
mod m20260710_000003_trade_intents;
mod m20260713_000001_automation_idempotency_key;
mod m20260713_000002_usernames;
mod m20260713_000003_automation_observations;
mod m20260713_000004_wallet_signature_challenges;
mod m20260713_000005_trade_submission_hardening;
mod m20260713_000006_polymarket_private_beta_lifecycle;
mod m20260718_000001_market_comments;
mod m20260718_000002_market_favorites;
