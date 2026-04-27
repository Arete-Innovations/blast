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
pub enum Role {
    Admin,
    Member,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }

    pub fn parse(s: &str) -> Result<Self, MeltDown> {
        match s {
            "admin" => Ok(Role::Admin),
            "member" => Ok(Role::Member),
            other => Err(MeltDown::validation_failed(format!("unknown role: {}", other))),
        }
    }
}

impl FromSql<UserRole, Pg> for Role {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"admin" => Ok(Role::Admin),
            b"member" => Ok(Role::Member),
            other => Err(format!("unknown role: {}", String::from_utf8_lossy(other)).into()),
        }
    }
}

impl ToSql<UserRole, Pg> for Role {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.as_str().as_bytes())?;
        Ok(IsNull::No)
    }
}
