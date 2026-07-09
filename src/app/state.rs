use crate::{
    auth::service::AuthService,
    automations::service::AutomationService,
    config::AppConfig,
    db::{Db, connect},
    notifications::service::NotificationService,
    polymarket::client::PolymarketClient,
    users::service::UserService,
};
use migration::Migrator;
use sea_orm::DbErr;
use sea_orm_migration::MigratorTrait;

#[derive(Clone)]
pub struct AppState {
    pub auth_service: AuthService,
    pub automation_service: AutomationService,
    pub db: Db,
    pub notification_service: NotificationService,
    pub polymarket_client: PolymarketClient,
    pub user_service: UserService,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Self, DbErr> {
        let db = connect(&config).await?;
        Migrator::up(&db, None).await?;

        let notification_service = NotificationService::new();

        Ok(Self {
            auth_service: AuthService::new(
                db.clone(),
                config.credential_encryption_key.clone(),
                config.app_base_url.clone(),
            ),
            automation_service: AutomationService::new(db.clone(), notification_service.clone()),
            db: db.clone(),
            notification_service,
            polymarket_client: PolymarketClient::new(&config),
            user_service: UserService::new(db),
        })
    }
}
