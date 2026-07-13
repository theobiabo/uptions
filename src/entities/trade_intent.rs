use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "trade_intents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub automation_id: Option<String>,
    pub provider: String,
    pub chain: String,
    pub chain_id: i64,
    pub market_id: String,
    pub market_title: String,
    pub token_id: String,
    pub outcome: String,
    pub side: String,
    pub order_type: String,
    pub execution_type: String,
    pub amount: f64,
    pub price: Option<f64>,
    pub wallet_address: String,
    pub status: String,
    pub signed_order: Option<Value>,
    pub signed_order_hash: Option<String>,
    pub defer_exec: bool,
    pub post_only: bool,
    pub provider_response: Option<Value>,
    pub provider_order_id: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub submitted_at: Option<DateTimeWithTimeZone>,
    pub submission_started_at: Option<DateTimeWithTimeZone>,
    pub reconciliation_checked_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
