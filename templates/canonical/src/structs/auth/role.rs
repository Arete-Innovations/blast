#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;

#[cfg(not(target_arch = "wasm32"))]
use diesel::{
    backend::Backend,
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    pg::Pg,
    serialize::{self, IsNull, Output, ToSql},
};
use serde::{Deserialize, Serialize};

use crate::meltdown::*;

#[cfg(not(target_arch = "wasm32"))]
use crate::database::schema::sql_types::UserRole;

#[cfg_attr(not(target_arch = "wasm32"), derive(AsExpression, FromSqlRow))]
#[cfg_attr(not(target_arch = "wasm32"), diesel(sql_type = UserRole))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[cfg(not(target_arch = "wasm32"))]
impl FromSql<UserRole, Pg> for Role {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"admin" => Ok(Role::Admin),
            b"member" => Ok(Role::Member),
            other => Err(format!("unknown role: {}", String::from_utf8_lossy(other)).into()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ToSql<UserRole, Pg> for Role {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.as_str().as_bytes())?;
        Ok(IsNull::No)
    }
}
