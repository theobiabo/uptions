use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
};
use uptions_backend::{
    auth::service::{hash_password, normalize_email, validate_password},
    config::AppConfig,
    db,
    entities::user,
    load_env,
    providers::types::DEFAULT_PROVIDER,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_env();

    let config = AppConfig::from_env();
    let db = db::connect(&config).await?;

    let email = normalize_email(&std::env::var("SEED_USER_EMAIL")?)?;
    let password = std::env::var("SEED_USER_PASSWORD")?;

    validate_password(&password)?;

    let password_hash = hash_password(&password)?;
    let now = Utc::now().into();

    if let Some(existing) = user::Entity::find()
        .filter(user::Column::Email.eq(Some(email.clone())))
        .one(&db)
        .await?
    {
        let mut active = existing.into_active_model();
        active.password_hash = Set(Some(password_hash));
        active.email_verified_at = Set(Some(now));
        active.preferred_trading_provider = Set(DEFAULT_PROVIDER.storage_value().to_owned());
        active.updated_at = Set(now);
        active.update(&db).await?;

        println!("updated user {}", email);
    } else {
        user::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            email: Set(Some(email.clone())),
            password_hash: Set(Some(password_hash)),
            email_verified_at: Set(Some(now)),
            preferred_trading_provider: Set(DEFAULT_PROVIDER.storage_value().to_owned()),
            ..Default::default()
        }
        .insert(&db)
        .await?;

        println!("created user {}", email);
    }

    Ok(())
}
