//! Bearer-token session middleware.
//!
//! Resolves `Authorization: Bearer <token>` headers against the `sessions`
//! table (joined to `users`), constructs a [`SessionContext`], and stuffs
//! it into request extensions so handlers can `Extension(ctx)` it.
//!
//! Cookies are not consulted — the canonical auth contract is bearer-only
//! to keep SPA + native clients on a single code path.

use axum::{extract::Request, http::header::AUTHORIZATION, middleware::Next, response::Response};
use diesel::sql_types::{Int8, Text};
use diesel_async::RunQueryDsl;

use crate::{
    cata_log,
    database::db::establish_connection,
    meltdown::*,
};

// Re-export for back-compat. New code should import `catalyst::SessionContext`
// (or `crate::sessions::SessionContext`) directly.
pub use crate::sessions::SessionContext;

/// Row shape returned by the resolve query: SELECT u.id, u.role, s.id ...
#[derive(diesel::QueryableByName, Debug)]
struct SessionResolveRow {
    #[diesel(sql_type = Int8)]
    user_id: i64,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Int8)]
    session_id: i64,
}

fn extract_token(request: &Request) -> Option<String> {
    let value = request.headers().get(AUTHORIZATION)?;
    let header = value.to_str().ok()?;
    let token = header.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

async fn resolve_session(raw_token: &str) -> Result<SessionContext, MeltDown> {
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
    .bind::<Text, _>(raw_token)
    .load(&mut conn)
    .await
    .map_err(|e| MeltDown::from(e).with_context("operation", "resolve_session"))?;

    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| MeltDown::session_invalid("Session token not recognised or expired"))?;

    cata_log!(Debug, format!("Authenticated user_id={}", row.user_id));
    Ok(SessionContext::new(
        row.session_id,
        row.user_id,
        row.role,
        raw_token,
    ))
}

pub async fn session_auth_middleware(mut request: Request, next: Next) -> Result<Response, MeltDown> {
    let raw_token = extract_token(&request).ok_or_else(|| {
        cata_log!(Debug, "Missing bearer token (no Authorization header)");
        MeltDown::session_missing()
    })?;

    let ctx = resolve_session(&raw_token).await?;
    request.extensions_mut().insert(ctx);

    Ok(next.run(request).await)
}

pub async fn admin_auth_middleware(mut request: Request, next: Next) -> Result<Response, MeltDown> {
    let raw_token = extract_token(&request).ok_or_else(|| {
        cata_log!(Debug, "Missing bearer token for admin route");
        MeltDown::session_missing()
    })?;

    let ctx = resolve_session(&raw_token).await?;

    if !ctx.is_admin() {
        cata_log!(Warning, format!("Non-admin user attempted admin access (user_id: {})", ctx.user_id));
        return Err(MeltDown::new(MeltType::Forbidden, "Admin access required"));
    }

    cata_log!(Debug, format!("Authenticated admin user (user_id: {})", ctx.user_id));
    request.extensions_mut().insert(ctx);

    Ok(next.run(request).await)
}
