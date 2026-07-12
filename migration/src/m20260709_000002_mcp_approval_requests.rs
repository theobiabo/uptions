use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(McpApprovalRequests::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(McpApprovalRequests::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(McpApprovalRequests::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpApprovalRequests::Tool)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpApprovalRequests::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(McpApprovalRequests::Payload)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(ColumnDef::new(McpApprovalRequests::Result).json_binary())
                    .col(
                        ColumnDef::new(McpApprovalRequests::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(McpApprovalRequests::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(McpApprovalRequests::DecidedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(McpApprovalRequests::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(McpApprovalRequests::Table, McpApprovalRequests::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(McpApprovalRequests::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum McpApprovalRequests {
    Table,
    Id,
    UserId,
    Tool,
    Status,
    Payload,
    Result,
    CreatedAt,
    UpdatedAt,
    DecidedAt,
    ExpiresAt,
}
