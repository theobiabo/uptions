use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Automations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Automations::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Automations::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Automations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Automations::MarketId).string_len(128))
                    .col(ColumnDef::new(Automations::MarketTitle).string_len(512))
                    .col(ColumnDef::new(Automations::Venue).string_len(64).not_null())
                    .col(
                        ColumnDef::new(Automations::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Automations::Workflow)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(ColumnDef::new(Automations::LastRunStatus).string_len(32))
                    .col(ColumnDef::new(Automations::LastRunAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Automations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Automations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Automations::Table, Automations::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AutomationAlerts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AutomationAlerts::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AutomationAlerts::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AutomationAlerts::AutomationId).string_len(36))
                    .col(
                        ColumnDef::new(AutomationAlerts::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AutomationAlerts::Message).text().not_null())
                    .col(
                        ColumnDef::new(AutomationAlerts::Status)
                            .string_len(32)
                            .not_null()
                            .default("info"),
                    )
                    .col(
                        ColumnDef::new(AutomationAlerts::Meta)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(AutomationAlerts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AutomationAlerts::Table, AutomationAlerts::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AutomationAlerts::Table, AutomationAlerts::AutomationId)
                            .to(Automations::Table, Automations::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AutomationAlerts::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Automations::Table).to_owned())
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
    UserId,
    Title,
    MarketId,
    MarketTitle,
    Venue,
    Status,
    Workflow,
    LastRunStatus,
    LastRunAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum AutomationAlerts {
    Table,
    Id,
    UserId,
    AutomationId,
    Title,
    Message,
    Status,
    Meta,
    CreatedAt,
}
