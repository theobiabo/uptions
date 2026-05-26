use crate::{
    db::{DbPool, get_connection},
    error::AppError,
};

pub struct JoinWaitlistStruct {
    pub name: String,
    pub email: String,
}

#[derive(Clone)]
pub struct UserService {
    db: DbPool,
}

impl UserService {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    pub async fn join_waitlist(&self, payload: JoinWaitlistStruct) -> Result<(), AppError> {
        let _ = payload;
        let _connection = get_connection(&self.db)?;

        Ok(())
    }
}
