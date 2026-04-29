use diesel::{Identifiable, Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

use crate::database::schema::sessions;

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
