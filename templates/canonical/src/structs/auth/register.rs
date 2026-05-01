use serde::Deserialize;

use crate::structs::{auth::SessionContext, UserPublic};

#[derive(Clone)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
}

pub struct RegisterOutput {
    pub token: String,
    pub user: UserPublic,
    pub session: SessionContext,
}
