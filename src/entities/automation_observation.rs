use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "automation_observations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub automation_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub step_id: String,
    pub value: Option<f64>,
    pub observed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
