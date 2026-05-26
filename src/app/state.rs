use crate::{
    auth::service::AuthService,
    config::AppConfig,
    db::{Db, connect},
    polymarket::client::PolymarketClient,
    users::service::UserService,
};
use migration::Migrator;
use sea_orm::DbErr;
use sea_orm_migration::MigratorTrait;

#[derive(Clone)]
pub struct AppState {
    pub auth_service: AuthService,
    pub db: Db,
    pub polymarket_client: PolymarketClient,
    pub user_service: UserService,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Self, DbErr> {
        let db = connect(&config).await?;
        Migrator::up(&db, None).await?;

        Ok(Self {
            auth_service: AuthService::new(),
            db: db.clone(),
            polymarket_client: PolymarketClient::new(&config),
            user_service: UserService::new(db),
        })
    }
}
