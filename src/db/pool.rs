use sea_orm::{Database, DatabaseConnection, DbErr};

use crate::config::AppConfig;

pub type Db = DatabaseConnection;

pub async fn connect(config: &AppConfig) -> Result<Db, DbErr> {
    Database::connect(&config.database_url).await
}
