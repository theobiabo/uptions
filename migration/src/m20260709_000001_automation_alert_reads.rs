use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AutomationAlerts::Table)
                    .add_column(ColumnDef::new(AutomationAlerts::ReadAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AutomationAlerts::Table)
                    .drop_column(AutomationAlerts::ReadAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AutomationAlerts {
    Table,
    ReadAt,
}
