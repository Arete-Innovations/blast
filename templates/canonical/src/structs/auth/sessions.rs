use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::database::schema::sessions;

#[derive(Debug, Clone, Queryable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = sessions)]
pub struct Session {
    pub id: i32,
    pub user_id: i32,
    pub token_hash: Vec<u8>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub revoked: bool,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = sessions)]
pub struct NewSession {
    pub user_id: i32,
    pub token_hash: Vec<u8>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub expires_at: i64,
}
