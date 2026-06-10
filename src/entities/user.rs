use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub primary_wallet_address: Option<String>,
    pub password_hash: Option<String>,
    pub email: Option<String>,
    pub email_verified_at: Option<DateTimeWithTimeZone>,
    pub email_verification_token_hash: Option<String>,
    pub email_verification_expires_at: Option<DateTimeWithTimeZone>,
    pub password_reset_token_hash: Option<String>,
    pub password_reset_expires_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
