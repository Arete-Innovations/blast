use serde::Serialize;

use crate::structs::UserPublic;

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserPublic,
}
