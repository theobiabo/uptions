use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketComments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketComments::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketComments::MarketId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketComments::AuthorId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarketComments::Body).text().not_null())
                    .col(
                        ColumnDef::new(MarketComments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketComments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MarketComments::Table, MarketComments::AuthorId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("char_length(body) BETWEEN 1 AND 2000"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("market_comments_market_created_idx")
                    .table(MarketComments::Table)
                    .col(MarketComments::MarketId)
                    .col(MarketComments::CreatedAt)
                    .col(MarketComments::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("market_comments_author_idx")
                    .table(MarketComments::Table)
                    .col(MarketComments::AuthorId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MarketComments::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum MarketComments {
    Table,
    Id,
    MarketId,
    AuthorId,
    Body,
    CreatedAt,
    UpdatedAt,
}
