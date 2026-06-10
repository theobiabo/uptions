use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ;
                ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verification_token_hash VARCHAR(64);
                ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verification_expires_at TIMESTAMPTZ;
                ALTER TABLE users ADD COLUMN IF NOT EXISTS password_reset_token_hash VARCHAR(64);
                ALTER TABLE users ADD COLUMN IF NOT EXISTS password_reset_expires_at TIMESTAMPTZ;
                CREATE INDEX IF NOT EXISTS users_email_verification_token_hash_idx ON users (email_verification_token_hash);
                CREATE INDEX IF NOT EXISTS users_password_reset_token_hash_idx ON users (password_reset_token_hash);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP INDEX IF EXISTS users_email_verification_token_hash_idx;
                DROP INDEX IF EXISTS users_password_reset_token_hash_idx;
                ALTER TABLE users DROP COLUMN IF EXISTS email_verified_at;
                ALTER TABLE users DROP COLUMN IF EXISTS email_verification_token_hash;
                ALTER TABLE users DROP COLUMN IF EXISTS email_verification_expires_at;
                ALTER TABLE users DROP COLUMN IF EXISTS password_reset_token_hash;
                ALTER TABLE users DROP COLUMN IF EXISTS password_reset_expires_at;
                "#,
            )
            .await?;

        Ok(())
    }
}
