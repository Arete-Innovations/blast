// AUTO-GENERATED from src/database/migrations/2026-04-26-000001_users_and_sessions/up.sql @ ed9086b614a858bbb4995100cbc1b7b39aa208f2f46ce2f10b82ea686eb46b5d
//
// Do not edit by hand. Run `blast gen all` after mutating state.

use std::io::Write;

use diesel::backend::Backend;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use serde::{Deserialize, Serialize};

use crate::database::schema::sql_types::UserRole;
use crate::meltdown::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsExpression, FromSqlRow, Serialize, Deserialize)]
#[diesel(sql_type = UserRole)]
pub enum UserRole {
    Admin,
    Member,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Member => "member",
        }
    }

    pub fn parse(s: &str) -> Result<Self, MeltDown> {
        match s {
            "admin" => Ok(UserRole::Admin),
            "member" => Ok(UserRole::Member),
            other => Err(MeltDown::validation_failed(format!("unknown user_role: {}", other))),
        }
    }
}

impl FromSql<UserRole, Pg> for UserRole {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"admin" => Ok(UserRole::Admin),
            b"member" => Ok(UserRole::Member),
            other => Err(format!("unknown user_role: {}", String::from_utf8_lossy(other)).into()),
        }
    }
}

impl ToSql<UserRole, Pg> for UserRole {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.as_str().as_bytes())?;
        Ok(IsNull::No)
    }
}
