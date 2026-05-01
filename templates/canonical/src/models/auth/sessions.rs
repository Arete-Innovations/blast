use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    database::schema::sessions::dsl as sessions_dsl,
    meltdown::*,
    structs::{NewSession, Session, User},
};

pub async fn insert_session(conn: &mut AsyncPgConnection, user_id: i64, token: &str, expires_at: i64) -> Result<Session, MeltDown> {
    let new_session = NewSession {
        user_id,
        token: token.to_string(),
        expires_at,
    };

    diesel::insert_into(sessions_dsl::sessions)
        .values(&new_session)
        .returning(Session::as_select())
        .get_result::<Session>(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "insert_session"))
}

pub async fn find_by_token(conn: &mut AsyncPgConnection, token: &str, now: i64) -> Result<Option<(Session, User)>, MeltDown> {
    use crate::database::schema::{sessions, users};

    sessions::table
        .inner_join(users::table)
        .filter(sessions::token.eq(token))
        .filter(sessions::expires_at.gt(now))
        .filter(users::deleted_at.is_null())
        .select((Session::as_select(), User::as_select()))
        .first::<(Session, User)>(conn)
        .await
        .optional()
        .map_err(|e| MeltDown::from(e).with_context("operation", "find_session_by_token"))
}

pub async fn delete_by_token(conn: &mut AsyncPgConnection, token: &str) -> Result<(), MeltDown> {
    diesel::delete(sessions_dsl::sessions.filter(sessions_dsl::token.eq(token)))
        .execute(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "delete_session_by_token"))?;
    Ok(())
}

