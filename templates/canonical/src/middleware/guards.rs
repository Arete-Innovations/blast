//! Axum extractor guards layered on top of `session_auth_middleware`.
//!
//! Use these as handler-arg extractors when you want a typed promise of
//! "this request is authenticated as X" rather than re-checking inside the
//! handler body.

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};
use diesel::sql_types::{Int8, Text};
use diesel_async::RunQueryDsl;

use crate::{
    cata_log,
    database::db::establish_connection,
    meltdown::*,
    middleware::auth_middleware::SessionContext,
};

pub struct AdminGuard(pub SessionContext);

#[async_trait]
impl<S> FromRequestParts<S> for AdminGuard
where
    S: Send + Sync,
{
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ctx = extract_session(parts).await?;
        if ctx.is_admin() {
            Ok(AdminGuard(ctx))
        } else {
            Err(MeltDown::new(MeltType::Forbidden, "Insufficient permissions to access admin area"))
        }
    }
}

pub struct UserGuard(pub SessionContext);

#[async_trait]
impl<S> FromRequestParts<S> for UserGuard
where
    S: Send + Sync,
{
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ctx = extract_session(parts).await?;
        Ok(UserGuard(ctx))
    }
}

#[derive(diesel::QueryableByName, Debug)]
struct SessionResolveRow {
    #[diesel(sql_type = Int8)]
    user_id: i64,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Int8)]
    session_id: i64,
}

async fn extract_session(parts: &Parts) -> Result<SessionContext, MeltDown> {
    use axum::http::header::AUTHORIZATION;

    let raw_token = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(str::trim).filter(|t| !t.is_empty()).map(str::to_string))
        .ok_or_else(MeltDown::session_missing)?;

    let mut conn = establish_connection().await?;
    let rows: Vec<SessionResolveRow> = diesel::sql_query(
        "SELECT u.id AS user_id, u.role AS role, s.id AS session_id \
         FROM sessions s \
         JOIN users u ON u.id = s.user_id \
         WHERE s.token = $1 \
           AND s.expires_at > extract(epoch from NOW())::bigint \
           AND u.deleted_at IS NULL \
         LIMIT 1",
    )
    .bind::<Text, _>(&raw_token)
    .load(&mut conn)
    .await
    .map_err(|e| MeltDown::from(e).with_context("operation", "extract_session_guard"))?;

    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| MeltDown::session_invalid("Session token not recognised or expired"))?;

    cata_log!(Debug, format!("Guard authenticated user_id={}", row.user_id));
    Ok(SessionContext::new(
        row.session_id,
        row.user_id,
        row.role,
        &raw_token,
    ))
}

pub struct Referer(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for Referer
where
    S: Send + Sync,
{
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.headers.get("Referer").and_then(|h| h.to_str().ok()) {
            Some(referer) => Ok(Referer(referer.to_string())),
            None => Err(MeltDown::new(MeltType::BadRequest, "Missing Referer header")),
        }
    }
}
