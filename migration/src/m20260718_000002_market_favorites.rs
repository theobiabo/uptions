use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketFavorites::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketFavorites::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketFavorites::UserId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketFavorites::MarketId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketFavorites::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MarketFavorites::Table, MarketFavorites::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("char_length(market_id) BETWEEN 1 AND 128"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("market_favorites_user_market_uidx")
                    .table(MarketFavorites::Table)
                    .col(MarketFavorites::UserId)
                    .col(MarketFavorites::MarketId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("market_favorites_user_created_idx")
                    .table(MarketFavorites::Table)
                    .col(MarketFavorites::UserId)
                    .col(MarketFavorites::CreatedAt)
                    .col(MarketFavorites::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MarketFavorites::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum MarketFavorites {
    Table,
    Id,
    UserId,
    MarketId,
    CreatedAt,
}
