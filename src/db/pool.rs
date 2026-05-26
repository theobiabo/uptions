use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool, PooledConnection},
};

use crate::config::AppConfig;

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = PooledConnection<ConnectionManager<PgConnection>>;

pub fn create_pool(config: &AppConfig) -> Result<DbPool, diesel::r2d2::PoolError> {
    let manager = ConnectionManager::<PgConnection>::new(config.database_url.clone());
    Pool::builder().build(manager)
}

pub fn get_connection(pool: &DbPool) -> Result<DbConnection, diesel::r2d2::PoolError> {
    pool.get()
}
