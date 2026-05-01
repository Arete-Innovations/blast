use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    database::schema::users::dsl as users_dsl,
    meltdown::*,
    structs::{auth::Role, NewUser, User},
};

pub async fn find_by_email(conn: &mut AsyncPgConnection, email: &str) -> Result<Option<User>, MeltDown> {
    users_dsl::users
        .filter(users_dsl::email.eq(email))
        .filter(users_dsl::deleted_at.is_null())
        .select(User::as_select())
        .first::<User>(conn)
        .await
        .optional()
        .map_err(|e| MeltDown::from(e).with_context("operation", "find_user_by_email"))
}

pub async fn find_by_id(conn: &mut AsyncPgConnection, id: i64) -> Result<Option<User>, MeltDown> {
    users_dsl::users
        .filter(users_dsl::id.eq(id))
        .filter(users_dsl::deleted_at.is_null())
        .select(User::as_select())
        .first::<User>(conn)
        .await
        .optional()
        .map_err(|e| MeltDown::from(e).with_context("operation", "find_user_by_id"))
}

pub async fn insert_new(conn: &mut AsyncPgConnection, email: &str, password_hash: &str) -> Result<User, MeltDown> {
    let new_user = NewUser {
        email: email.to_string(),
        password_hash: password_hash.to_string(),
    };

    diesel::insert_into(users_dsl::users)
        .values(&new_user)
        .returning(User::as_select())
        .get_result::<User>(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "insert_new_user"))
}

pub async fn set_role(conn: &mut AsyncPgConnection, id: i64, role: Role) -> Result<User, MeltDown> {
    diesel::update(users_dsl::users.filter(users_dsl::id.eq(id)))
        .set(users_dsl::role.eq(role))
        .returning(User::as_select())
        .get_result::<User>(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "set_user_role"))
}
