use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::database::schema::sessions;

/// Opaque session row. `token` is the raw bearer token (not hashed) —
/// catalyst's session table is a simple lookup table, not a credential
/// store. Tokens are minted client-side opaque, 32 random bytes
/// base64url-encoded.
#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = sessions)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = sessions)]
pub struct NewSession {
    pub user_id: i64,
    pub token: String,
    pub expires_at: i64,
}
