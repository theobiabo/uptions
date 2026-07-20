use sea_orm::entity::prelude::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "polymarket_user_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub venue_connection_id: String,
    pub trade_intent_id: Option<String>,
    pub event_kind: String,
    pub provider_event_id: String,
    pub event_identity: String,
    pub provider_order_id: Option<String>,
    pub provider_trade_id: Option<String>,
    pub status: Option<String>,
    pub market_id: Option<String>,
    pub token_id: Option<String>,
    pub provider_timestamp: Option<String>,
    pub payload: Value,
    pub received_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
