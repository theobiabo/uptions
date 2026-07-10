use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AutomationRuns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AutomationRuns::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AutomationRuns::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AutomationRuns::AutomationId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AutomationRuns::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AutomationRuns::TriggerSnapshot)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(AutomationRuns::ConditionSnapshot)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(AutomationRuns::ActionSnapshot)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(ColumnDef::new(AutomationRuns::Result).json_binary())
                    .col(ColumnDef::new(AutomationRuns::Error).text())
                    .col(
                        ColumnDef::new(AutomationRuns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(AutomationRuns::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(AutomationRuns::Table, AutomationRuns::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AutomationRuns::Table, AutomationRuns::AutomationId)
                            .to(Automations::Table, Automations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AutomationRuns::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Automations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum AutomationRuns {
    Table,
    Id,
    UserId,
    AutomationId,
    Status,
    TriggerSnapshot,
    ConditionSnapshot,
    ActionSnapshot,
    Result,
    Error,
    CreatedAt,
    CompletedAt,
}
