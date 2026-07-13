use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AutomationObservations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AutomationObservations::AutomationId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AutomationObservations::StepId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AutomationObservations::Value).double())
                    .col(
                        ColumnDef::new(AutomationObservations::ObservedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(AutomationObservations::AutomationId)
                            .col(AutomationObservations::StepId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                AutomationObservations::Table,
                                AutomationObservations::AutomationId,
                            )
                            .to(Automations::Table, Automations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(AutomationObservations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Automations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum AutomationObservations {
    Table,
    AutomationId,
    StepId,
    Value,
    ObservedAt,
}
