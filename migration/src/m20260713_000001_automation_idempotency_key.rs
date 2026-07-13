use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Automations::Table)
                    .add_column(ColumnDef::new(Automations::IdempotencyKey).string_len(36))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("automations_user_idempotency_key_uidx")
                    .table(Automations::Table)
                    .col(Automations::UserId)
                    .col(Automations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("automations_user_idempotency_key_uidx")
                    .table(Automations::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Automations::Table)
                    .drop_column(Automations::IdempotencyKey)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Automations {
    Table,
    UserId,
    IdempotencyKey,
}
