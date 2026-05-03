use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub struct AuthService;

impl AuthService {
    async fn register() -> Result<(), AppError>{

    }

    async  fn sign_in() -> Result<(), AppError>{

    }
}
