use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::{
    database::{db::establish_connection, schema::sessions::dsl as session_dsl},
    meltdown::*,
    services::crypto,
    structs::{NewSession, Session},
};

pub const DEFAULT_SESSION_DAYS: i64 = 30;

pub async fn create_session(
    user_id: i32,
    user_agent: Option<String>,
    ip: Option<String>,
) -> Result<(Session, String), MeltDown> {
    let mut conn = establish_connection().await?;

    let raw_token = crypto::generate_session_token();
    let token_hash = crypto::sha256(&raw_token);
    let expires_at = (Utc::now() + Duration::days(DEFAULT_SESSION_DAYS)).timestamp();

    let new_session = NewSession {
        user_id,
        token_hash,
        user_agent,
        ip,
        expires_at,
    };

    let session = diesel::insert_into(session_dsl::sessions)
        .values(&new_session)
        .get_result::<Session>(&mut conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "create_session"))?;

    Ok((session, raw_token))
}

pub async fn find_by_token(raw_token: &str) -> Result<Option<Session>, MeltDown> {
    let mut conn = establish_connection().await?;
    let token_hash = crypto::sha256(raw_token);

    session_dsl::sessions
        .filter(session_dsl::token_hash.eq(token_hash))
        .filter(session_dsl::revoked.eq(false))
        .first::<Session>(&mut conn)
        .await
        .optional()
        .map_err(|e| MeltDown::from(e).with_context("operation", "find_session_by_token"))
}

pub async fn revoke(id: i32) -> Result<(), MeltDown> {
    let mut conn = establish_connection().await?;

    diesel::update(session_dsl::sessions.filter(session_dsl::id.eq(id)))
        .set(session_dsl::revoked.eq(true))
        .execute(&mut conn)
        .await
        .map_err(|e| {
            MeltDown::from(e)
                .with_context("operation", "revoke_session")
                .with_context("session_id", id.to_string())
        })?;
    Ok(())
}

pub async fn revoke_all_for_user(user_id: i32) -> Result<(), MeltDown> {
    let mut conn = establish_connection().await?;

    diesel::update(
        session_dsl::sessions
            .filter(session_dsl::user_id.eq(user_id))
            .filter(session_dsl::revoked.eq(false)),
    )
    .set(session_dsl::revoked.eq(true))
    .execute(&mut conn)
    .await
    .map_err(|e| {
        MeltDown::from(e)
            .with_context("operation", "revoke_all_sessions_for_user")
            .with_context("user_id", user_id.to_string())
    })?;
    Ok(())
}

pub async fn touch_last_seen(id: i32) -> Result<(), MeltDown> {
    let mut conn = establish_connection().await?;
    let now = Utc::now().timestamp();

    diesel::update(session_dsl::sessions.filter(session_dsl::id.eq(id)))
        .set(session_dsl::last_seen_at.eq(now))
        .execute(&mut conn)
        .await
        .map_err(|e| {
            MeltDown::from(e)
                .with_context("operation", "touch_last_seen")
                .with_context("session_id", id.to_string())
        })?;
    Ok(())
}
