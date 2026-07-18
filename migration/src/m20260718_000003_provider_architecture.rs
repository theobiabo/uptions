use sea_orm_migration::prelude::*;

const UP_SQL: &str = r#"
UPDATE users
SET preferred_trading_provider = 'POLYMARKET'
WHERE preferred_trading_provider IS NULL
   OR upper(preferred_trading_provider) = 'POLYMARKET';
ALTER TABLE users
    ALTER COLUMN preferred_trading_provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN preferred_trading_provider SET NOT NULL,
    ADD CONSTRAINT users_preferred_trading_provider_check
        CHECK (preferred_trading_provider = 'POLYMARKET');

-- Legacy columns and indexes remain writable until a later contract migration.
ALTER TABLE venue_connections ADD COLUMN provider varchar(64);
UPDATE venue_connections SET provider = 'POLYMARKET' WHERE provider IS NULL;
ALTER TABLE venue_connections
    ALTER COLUMN provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN provider SET NOT NULL,
    ADD CONSTRAINT venue_connections_provider_check CHECK (provider = 'POLYMARKET');
CREATE UNIQUE INDEX venue_connections_user_provider_uidx
    ON venue_connections (user_id, provider);

ALTER TABLE automations ALTER COLUMN venue SET DEFAULT 'polymarket';
ALTER TABLE automations
    ADD COLUMN provider varchar(64),
    ADD COLUMN chain varchar(64),
    ADD COLUMN chain_id bigint;
UPDATE automations
SET provider = 'POLYMARKET', chain = 'POLYGON', chain_id = 137;
ALTER TABLE automations
    ALTER COLUMN provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN provider SET NOT NULL,
    ALTER COLUMN chain SET DEFAULT 'POLYGON',
    ALTER COLUMN chain SET NOT NULL,
    ALTER COLUMN chain_id SET DEFAULT 137,
    ALTER COLUMN chain_id SET NOT NULL,
    ADD CONSTRAINT automations_provider_chain_check
        CHECK (provider = 'POLYMARKET' AND chain = 'POLYGON' AND chain_id = 137);
CREATE INDEX automations_provider_market_idx
    ON automations (provider, market_id);

ALTER TABLE wallet_signature_challenges
    ADD COLUMN provider varchar(64),
    ADD COLUMN chain varchar(64);
UPDATE wallet_signature_challenges
SET provider = 'POLYMARKET', chain = 'POLYGON';
ALTER TABLE wallet_signature_challenges
    ALTER COLUMN provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN provider SET NOT NULL,
    ALTER COLUMN chain SET DEFAULT 'POLYGON',
    ALTER COLUMN chain SET NOT NULL,
    ADD CONSTRAINT wallet_signature_challenges_provider_chain_check
        CHECK (provider = 'POLYMARKET' AND chain = 'POLYGON' AND chain_id = 137);
CREATE INDEX wallet_signature_challenges_user_provider_chain_purpose_used_idx
    ON wallet_signature_challenges (user_id, provider, chain_id, purpose, used_at);

ALTER TABLE market_comments ADD COLUMN provider varchar(64);
UPDATE market_comments SET provider = 'POLYMARKET';
ALTER TABLE market_comments
    ALTER COLUMN provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN provider SET NOT NULL,
    ADD CONSTRAINT market_comments_provider_check CHECK (provider = 'POLYMARKET');
CREATE INDEX market_comments_provider_market_created_idx
    ON market_comments (provider, market_id, created_at, id);

ALTER TABLE market_favorites ADD COLUMN provider varchar(64);
UPDATE market_favorites SET provider = 'POLYMARKET';
ALTER TABLE market_favorites
    ALTER COLUMN provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN provider SET NOT NULL,
    ADD CONSTRAINT market_favorites_provider_check CHECK (provider = 'POLYMARKET');
CREATE UNIQUE INDEX market_favorites_user_provider_market_uidx
    ON market_favorites (user_id, provider, market_id);
CREATE INDEX market_favorites_user_provider_created_idx
    ON market_favorites (user_id, provider, created_at, id);

ALTER TABLE polymarket_user_events ADD COLUMN provider varchar(64);
UPDATE polymarket_user_events SET provider = 'POLYMARKET';
ALTER TABLE polymarket_user_events
    ALTER COLUMN provider SET DEFAULT 'POLYMARKET',
    ALTER COLUMN provider SET NOT NULL,
    ADD CONSTRAINT polymarket_user_events_provider_check
        CHECK (provider = 'POLYMARKET');
CREATE UNIQUE INDEX polymarket_user_events_provider_identity_uidx
    ON polymarket_user_events (provider, event_identity);

UPDATE trade_intents
SET provider = upper(provider), chain = upper(chain);
ALTER TABLE trade_intents
    ADD CONSTRAINT trade_intents_provider_chain_check
        CHECK (provider = 'POLYMARKET' AND chain = 'POLYGON' AND chain_id = 137);
CREATE INDEX trade_intents_provider_order_id_scoped_idx
    ON trade_intents (provider, provider_order_id);
CREATE INDEX trade_intents_provider_market_instrument_idx
    ON trade_intents (provider, market_id, token_id);
CREATE UNIQUE INDEX trade_intents_provider_signed_order_hash_uidx
    ON trade_intents (provider, signed_order_hash);
"#;

const DOWN_SQL: &str = r#"
DROP INDEX IF EXISTS trade_intents_provider_signed_order_hash_uidx;
DROP INDEX IF EXISTS trade_intents_provider_market_instrument_idx;
DROP INDEX IF EXISTS trade_intents_provider_order_id_scoped_idx;
ALTER TABLE trade_intents
    DROP CONSTRAINT trade_intents_provider_chain_check;

DROP INDEX IF EXISTS polymarket_user_events_provider_identity_uidx;
ALTER TABLE polymarket_user_events
    DROP CONSTRAINT polymarket_user_events_provider_check,
    DROP COLUMN provider;

DROP INDEX IF EXISTS market_favorites_user_provider_created_idx;
DROP INDEX IF EXISTS market_favorites_user_provider_market_uidx;
ALTER TABLE market_favorites
    DROP CONSTRAINT market_favorites_provider_check,
    DROP COLUMN provider;

DROP INDEX IF EXISTS market_comments_provider_market_created_idx;
ALTER TABLE market_comments
    DROP CONSTRAINT market_comments_provider_check,
    DROP COLUMN provider;

DROP INDEX IF EXISTS wallet_signature_challenges_user_provider_chain_purpose_used_idx;
ALTER TABLE wallet_signature_challenges
    DROP CONSTRAINT wallet_signature_challenges_provider_chain_check,
    DROP COLUMN chain,
    DROP COLUMN provider;

DROP INDEX IF EXISTS automations_provider_market_idx;
ALTER TABLE automations
    DROP CONSTRAINT automations_provider_chain_check,
    DROP COLUMN chain_id,
    DROP COLUMN chain,
    DROP COLUMN provider;

DROP INDEX IF EXISTS venue_connections_user_provider_uidx;
ALTER TABLE venue_connections
    DROP CONSTRAINT venue_connections_provider_check,
    DROP COLUMN provider;

ALTER TABLE users
    DROP CONSTRAINT users_preferred_trading_provider_check,
    ALTER COLUMN preferred_trading_provider DROP NOT NULL,
    ALTER COLUMN preferred_trading_provider DROP DEFAULT;
"#;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP_SQL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DOWN_SQL)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DOWN_SQL, UP_SQL};

    #[test]
    fn expand_keeps_legacy_venue_columns_and_indexes() {
        assert!(!UP_SQL.contains("RENAME COLUMN venue"));
        assert!(!UP_SQL.contains("UPDATE venue_connections SET venue"));
        assert!(!UP_SQL.contains("DROP INDEX"));
        assert!(
            UP_SQL.contains("ALTER TABLE automations ALTER COLUMN venue SET DEFAULT 'polymarket'")
        );
        assert!(UP_SQL.contains("ALTER TABLE venue_connections ADD COLUMN provider"));
        assert!(UP_SQL.contains("CREATE UNIQUE INDEX venue_connections_user_provider_uidx"));
    }

    #[test]
    fn contract_removes_only_new_provider_storage_from_legacy_tables() {
        assert!(!DOWN_SQL.contains("RENAME COLUMN provider TO venue"));
        assert!(!DOWN_SQL.contains("UPDATE venue_connections SET venue"));
        assert!(DOWN_SQL.contains("DROP COLUMN provider"));
        assert!(!DOWN_SQL.contains("DROP COLUMN venue"));
    }
}
