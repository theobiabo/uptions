use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TradeIntents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TradeIntents::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(ColumnDef::new(TradeIntents::AutomationId).string_len(36))
                    .col(
                        ColumnDef::new(TradeIntents::Provider)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::Chain)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::MarketId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(TradeIntents::MarketTitle).text().not_null())
                    .col(ColumnDef::new(TradeIntents::TokenId).text().not_null())
                    .col(
                        ColumnDef::new(TradeIntents::Outcome)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(TradeIntents::Side).string_len(16).not_null())
                    .col(
                        ColumnDef::new(TradeIntents::OrderType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::ExecutionType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(TradeIntents::Amount).double().not_null())
                    .col(ColumnDef::new(TradeIntents::Price).double())
                    .col(
                        ColumnDef::new(TradeIntents::WalletAddress)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(TradeIntents::SignedOrder).json_binary())
                    .col(ColumnDef::new(TradeIntents::ProviderResponse).json_binary())
                    .col(ColumnDef::new(TradeIntents::ProviderOrderId).text())
                    .col(ColumnDef::new(TradeIntents::Error).text())
                    .col(
                        ColumnDef::new(TradeIntents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TradeIntents::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(TradeIntents::SubmittedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(TradeIntents::Table, TradeIntents::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("trade_intents_user_updated_idx")
                    .table(TradeIntents::Table)
                    .col(TradeIntents::UserId)
                    .col(TradeIntents::UpdatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TradeIntents::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TradeIntents {
    Table,
    Id,
    UserId,
    AutomationId,
    Provider,
    Chain,
    ChainId,
    MarketId,
    MarketTitle,
    TokenId,
    Outcome,
    Side,
    OrderType,
    ExecutionType,
    Amount,
    Price,
    WalletAddress,
    Status,
    SignedOrder,
    ProviderResponse,
    ProviderOrderId,
    Error,
    CreatedAt,
    UpdatedAt,
    SubmittedAt,
}
