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
                    .add_column(ColumnDef::new(TradeIntents::SignedOrderHash).string_len(64))
                    .add_column(
                        ColumnDef::new(TradeIntents::DeferExec)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .add_column(
                        ColumnDef::new(TradeIntents::PostOnly)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .add_column(
                        ColumnDef::new(TradeIntents::SubmissionStartedAt)
                            .timestamp_with_time_zone(),
                    )
                    .add_column(
                        ColumnDef::new(TradeIntents::ReconciliationCheckedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("trade_intents_signed_order_hash_uidx")
                    .table(TradeIntents::Table)
                    .col(TradeIntents::SignedOrderHash)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("trade_intents_signed_order_hash_uidx")
                    .table(TradeIntents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(TradeIntents::Table)
                    .drop_column(TradeIntents::ReconciliationCheckedAt)
                    .drop_column(TradeIntents::SubmissionStartedAt)
                    .drop_column(TradeIntents::PostOnly)
                    .drop_column(TradeIntents::DeferExec)
                    .drop_column(TradeIntents::SignedOrderHash)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum TradeIntents {
    Table,
    SignedOrderHash,
    DeferExec,
    PostOnly,
    SubmissionStartedAt,
    ReconciliationCheckedAt,
}
