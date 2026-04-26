use diesel::{prelude::*, Queryable};
use serde::{Deserialize, Serialize};

use crate::database::schema::users;

/// Full users row as stored in Postgres. Includes the password hash —
/// never serialize this directly to the client; project to `UserPublic`.
#[derive(Queryable, QueryableByName, Selectable, Debug, Clone, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

/// Insert payload for `users`. The `role`/`created_at`/`updated_at` columns
/// fall back to their DB defaults when omitted.
#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
}

/// Public projection of a user row — safe to send over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: i64,
    pub email: String,
    pub role: String,
}

impl From<&User> for UserPublic {
    fn from(u: &User) -> Self {
        Self {
            id: u.id,
            email: u.email.clone(),
            role: u.role.clone(),
        }
    }
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            role: u.role,
        }
    }
}
