use crate::{
    auth::service::AuthService,
    config::AppConfig,
    db::{DbPool, create_pool},
    polymarket::client::PolymarketClient,
    users::service::UserService,
};

#[derive(Clone)]
pub struct AppState {
    pub auth_service: AuthService,
    pub db: DbPool,
    pub polymarket_client: PolymarketClient,
    pub user_service: UserService,
}

impl AppState {
    pub fn new(config: AppConfig) -> Result<Self, diesel::r2d2::PoolError> {
        let db = create_pool(&config)?;

        Ok(Self {
            auth_service: AuthService::new(),
            db: db.clone(),
            polymarket_client: PolymarketClient::new(&config),
            user_service: UserService::new(db),
        })
    }
}
