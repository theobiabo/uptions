use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TradeIntents::Table)
                    .add_column(ColumnDef::new(TradeIntents::SignedMakerAmountBase).text())
                    .add_column(ColumnDef::new(TradeIntents::SignedTakerAmountBase).text())
                    .add_column(ColumnDef::new(TradeIntents::NormalizedAmountBase).text())
                    .add_column(ColumnDef::new(TradeIntents::NormalizedPriceNumerator).text())
                    .add_column(ColumnDef::new(TradeIntents::NormalizedPriceDenominator).text())
                    .add_column(
                        ColumnDef::new(TradeIntents::CancellationRequestedAt)
                            .timestamp_with_time_zone(),
                    )
                    .add_column(
                        ColumnDef::new(TradeIntents::CancelledAt).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE trade_intents SET status = 'matched' WHERE status = 'filled'",
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("trade_intents_provider_order_id_idx")
                    .table(TradeIntents::Table)
                    .col(TradeIntents::ProviderOrderId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PolymarketUserEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PolymarketUserEvents::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PolymarketUserEvents::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolymarketUserEvents::VenueConnectionId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PolymarketUserEvents::TradeIntentId).string_len(36))
                    .col(
                        ColumnDef::new(PolymarketUserEvents::EventKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolymarketUserEvents::ProviderEventId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolymarketUserEvents::EventIdentity)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PolymarketUserEvents::ProviderOrderId).text())
                    .col(ColumnDef::new(PolymarketUserEvents::ProviderTradeId).text())
                    .col(ColumnDef::new(PolymarketUserEvents::Status).string_len(64))
                    .col(ColumnDef::new(PolymarketUserEvents::MarketId).text())
                    .col(ColumnDef::new(PolymarketUserEvents::TokenId).text())
                    .col(ColumnDef::new(PolymarketUserEvents::ProviderTimestamp).text())
                    .col(
                        ColumnDef::new(PolymarketUserEvents::Payload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolymarketUserEvents::ReceivedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PolymarketUserEvents::Table, PolymarketUserEvents::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                PolymarketUserEvents::Table,
                                PolymarketUserEvents::VenueConnectionId,
                            )
                            .to(VenueConnections::Table, VenueConnections::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                PolymarketUserEvents::Table,
                                PolymarketUserEvents::TradeIntentId,
                            )
                            .to(TradeIntents::Table, TradeIntents::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("polymarket_user_events_identity_uidx")
                    .table(PolymarketUserEvents::Table)
                    .col(PolymarketUserEvents::EventIdentity)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("polymarket_user_events_trade_received_idx")
                    .table(PolymarketUserEvents::Table)
                    .col(PolymarketUserEvents::TradeIntentId)
                    .col(PolymarketUserEvents::ReceivedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PolymarketUserEvents::Table).to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("trade_intents_provider_order_id_idx")
                    .table(TradeIntents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(TradeIntents::Table)
                    .drop_column(TradeIntents::CancelledAt)
                    .drop_column(TradeIntents::CancellationRequestedAt)
                    .drop_column(TradeIntents::NormalizedPriceDenominator)
                    .drop_column(TradeIntents::NormalizedPriceNumerator)
                    .drop_column(TradeIntents::NormalizedAmountBase)
                    .drop_column(TradeIntents::SignedTakerAmountBase)
                    .drop_column(TradeIntents::SignedMakerAmountBase)
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
enum VenueConnections {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TradeIntents {
    Table,
    Id,
    SignedMakerAmountBase,
    SignedTakerAmountBase,
    NormalizedAmountBase,
    NormalizedPriceNumerator,
    NormalizedPriceDenominator,
    CancellationRequestedAt,
    CancelledAt,
    ProviderOrderId,
}

#[derive(DeriveIden)]
enum PolymarketUserEvents {
    Table,
    Id,
    UserId,
    VenueConnectionId,
    TradeIntentId,
    EventKind,
    ProviderEventId,
    EventIdentity,
    ProviderOrderId,
    ProviderTradeId,
    Status,
    MarketId,
    TokenId,
    ProviderTimestamp,
    Payload,
    ReceivedAt,
}
