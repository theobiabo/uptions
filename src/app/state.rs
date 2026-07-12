use crate::{
    auth::service::AuthService,
    automations::{executor::AutomationExecutor, service::AutomationService},
    config::AppConfig,
    db::{Db, connect},
    notifications::service::NotificationService,
    polymarket::client::PolymarketClient,
    trades::service::TradeService,
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
    pub trade_service: TradeService,
    pub user_service: UserService,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Self, DbErr> {
        let db = connect(&config).await?;
        Migrator::up(&db, None).await?;

        let notification_service = NotificationService::new();

        let automation_service = AutomationService::new(db.clone(), notification_service.clone());
        AutomationExecutor::new(
            db.clone(),
            automation_service.clone(),
            PolymarketClient::new(&config),
        )
        .start();
        let polymarket_client = PolymarketClient::new(&config);
        let trade_service = TradeService::new(
            db.clone(),
            polymarket_client.clone(),
            config.credential_encryption_key.clone(),
        );

        Ok(Self {
            auth_service: AuthService::new(
                db.clone(),
                config.credential_encryption_key.clone(),
                config.app_base_url.clone(),
            ),
            automation_service,
            db: db.clone(),
            notification_service,
            polymarket_client,
            trade_service,
            user_service: UserService::new(db),
        })
    }
}
