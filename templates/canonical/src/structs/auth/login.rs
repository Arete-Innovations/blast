use serde::{Deserialize, Serialize};

use crate::structs::{auth::SessionContext, UserPublic};

#[derive(Clone)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

pub struct LoginOutput {
    pub token: String,
    pub user: UserPublic,
    pub session: SessionContext,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserPublic,
}
