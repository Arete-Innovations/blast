use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    cata_log,
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
        .get_result::<Session>(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "insert_session"))
}

pub async fn find_by_token(conn: &mut AsyncPgConnection, token: &str) -> Result<Option<(Session, User)>, MeltDown> {
    use crate::database::schema::{sessions, users};

    let now = now_unix();

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

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => {
            cata_log!(Error, format!("system clock before epoch: {}", e));
            0
        }
    }
}
