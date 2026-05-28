use crate::{db::Db, entities::waitlist, error::AppError};
use sea_orm::{ActiveModelTrait, Set};

pub struct JoinWaitlistStruct {
    pub email: String,
}

#[derive(Clone)]
pub struct UserService {
    db: Db,
}

impl UserService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn join_waitlist(&self, payload: JoinWaitlistStruct) -> Result<(), AppError> {
        waitlist::ActiveModel {
            email: Set(payload.email),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }
}
