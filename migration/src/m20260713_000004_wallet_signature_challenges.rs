use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WalletSignatureChallenges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::Purpose)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::WalletAddress)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::NonceHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::UsedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(WalletSignatureChallenges::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                WalletSignatureChallenges::Table,
                                WalletSignatureChallenges::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("wallet_signature_challenges_user_purpose_used_idx")
                    .table(WalletSignatureChallenges::Table)
                    .col(WalletSignatureChallenges::UserId)
                    .col(WalletSignatureChallenges::Purpose)
                    .col(WalletSignatureChallenges::UsedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(WalletSignatureChallenges::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WalletSignatureChallenges {
    Table,
    Id,
    UserId,
    Purpose,
    ChainId,
    WalletAddress,
    NonceHash,
    ExpiresAt,
    UsedAt,
    CreatedAt,
    UpdatedAt,
}
