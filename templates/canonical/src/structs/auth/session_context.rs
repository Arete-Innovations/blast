use serde::{Deserialize, Serialize};

use crate::structs::auth::Role;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: i64,
    pub user_id: i64,
    pub role: Role,
    pub token: String,
}

impl SessionContext {
    pub fn new(session_id: i64, user_id: i64, role: Role, token: impl Into<String>) -> Self {
        Self {
            session_id,
            user_id,
            role,
            token: token.into(),
        }
    }
}
