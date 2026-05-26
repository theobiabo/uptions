use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Waitlist::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Waitlist::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Waitlist::Name).string_len(255).not_null())
                    .col(ColumnDef::new(Waitlist::Email).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Waitlist::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Waitlist::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Waitlist {
    Table,
    Id,
    Name,
    Email,
    CreatedAt,
}
