pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260527_131801_create_waitlist::Migration),
            Box::new(m20260601_000001_identity_and_venue_connections::Migration),
            Box::new(m20260609_000001_email_auth_and_sessions::Migration),
        ]
    }
}
mod m20260527_131801_create_waitlist;
mod m20260601_000001_identity_and_venue_connections;
mod m20260609_000001_email_auth_and_sessions;
