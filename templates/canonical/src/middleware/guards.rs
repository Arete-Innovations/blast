use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};
use diesel::sql_types::{Bool, Text};

use crate::{meltdown::*, middleware::auth_middleware::SessionContext, structs::*};

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

/// Minimal user identity fetched from the users table without depending on any
/// concrete `Users` struct.
#[derive(diesel::QueryableByName, Debug)]
struct SessionUserRow {
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Bool)]
    active: bool,
}

async fn extract_session(parts: &Parts) -> Result<SessionContext, MeltDown> {
    use axum::http::header::AUTHORIZATION;
    use axum_extra::extract::CookieJar;
    use chrono::Utc;
    use diesel_async::RunQueryDsl;

    use crate::{cata_log, database::db::establish_connection, models::sessions};

    let raw_token = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(str::trim).filter(|t| !t.is_empty()).map(str::to_string))
        .or_else(|| {
            CookieJar::from_headers(&parts.headers)
                .get(crate::middleware::auth_middleware::SESSION_COOKIE)
                .map(|c| c.value().to_string())
        })
        .ok_or_else(MeltDown::session_missing)?;

    let session = sessions::find_by_token(&raw_token)
        .await?
        .ok_or_else(|| MeltDown::session_invalid("Session token not recognised"))?;

    if session.revoked {
        return Err(MeltDown::session_invalid("Session revoked"));
    }

    let now = Utc::now().timestamp();
    if session.expires_at < now {
        return Err(MeltDown::session_expired());
    }

    let sid = session.id;
    tokio::spawn(async move {
        if let Err(e) = sessions::touch_last_seen(sid).await {
            cata_log!(Debug, format!("touch_last_seen({}) failed: {}", sid, e.log_message()));
        }
    });

    let mut conn = establish_connection().await?;
    let rows: Vec<SessionUserRow> = diesel::sql_query("SELECT role, active FROM users WHERE id = $1")
        .bind::<diesel::sql_types::Int4, _>(session.user_id)
        .load(&mut conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "extract_session_user"))?;

    let user = rows.into_iter().next().ok_or_else(|| MeltDown::session_invalid("Session user no longer exists"))?;

    if !user.active {
        return Err(MeltDown::new(MeltType::Unauthorized, "Account is inactive"));
    }

    Ok(SessionContext::new(session.id, session.user_id, user.role))
}

pub struct ApiKeyGuard(pub ApiKeys);

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyGuard
where
    S: Send + Sync,
{
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts.headers.get("Authorization").and_then(|h| h.to_str().ok());

        match auth_header {
            Some(value) if value.starts_with("Bearer ") => {
                let token = value.trim_start_matches("Bearer ").trim();

                if token.is_empty() {
                    let error = MeltDown::new(MeltType::Unauthorized, "Empty API key provided");
                    return Err(error);
                }

                match ApiKeys::validate_token(token).await {
                    Ok(api_key) => Ok(ApiKeyGuard(api_key)),
                    Err(_) => {
                        let error = MeltDown::new(MeltType::Forbidden, "Invalid API key");
                        Err(error)
                    }
                }
            }
            _ => {
                let error = MeltDown::new(MeltType::Unauthorized, "Missing Authorization header");
                Err(error)
            }
        }
    }
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
            None => {
                let error = MeltDown::new(MeltType::BadRequest, "Missing Referer header");
                Err(error)
            }
        }
    }
}
