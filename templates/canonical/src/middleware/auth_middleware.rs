use axum::{extract::Request, http::header::AUTHORIZATION, middleware::Next, response::Response};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use diesel::sql_types::{Bool, Text};
use diesel_async::RunQueryDsl;

use crate::{
    cata_log,
    database::db::establish_connection,
    meltdown::*,
    models::sessions,
};

// Re-export for back-compat. New code should import `catalyst::SessionContext`
// (or `crate::sessions::SessionContext`) directly.
pub use crate::sessions::SessionContext;

pub const SESSION_COOKIE: &str = "cb_session";

/// Minimal user identity fetched from the users table without depending on any
/// concrete `Users` struct. Only the fields the session middleware cares about.
#[derive(diesel::QueryableByName, Debug)]
struct SessionUserRow {
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Bool)]
    active: bool,
}

fn extract_token(request: &Request) -> Option<String> {
    if let Some(value) = request.headers().get(AUTHORIZATION) {
        if let Ok(header) = value.to_str() {
            if let Some(token) = header.strip_prefix("Bearer ") {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    let jar = CookieJar::from_headers(request.headers());
    jar.get(SESSION_COOKIE).map(|c| c.value().to_string())
}

async fn fetch_session_user(user_id: i32) -> Result<SessionUserRow, MeltDown> {
    let mut conn = establish_connection().await?;
    let rows: Vec<SessionUserRow> = diesel::sql_query("SELECT role, active FROM users WHERE id = $1")
        .bind::<diesel::sql_types::Int4, _>(user_id)
        .load(&mut conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "fetch_session_user"))?;

    rows.into_iter().next().ok_or_else(|| MeltDown::session_invalid("Session user no longer exists"))
}

async fn authenticate(raw_token: &str) -> Result<SessionContext, MeltDown> {
    let session = sessions::find_by_token(raw_token)
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

    let user = fetch_session_user(session.user_id).await.map_err(|e| {
        cata_log!(Warning, format!("Session {} refers to missing user {}: {}", session.id, session.user_id, e.log_message()));
        MeltDown::session_invalid("Session user no longer exists")
    })?;

    if !user.active {
        cata_log!(Warning, format!("Inactive user attempted access (ID: {})", session.user_id));
        return Err(MeltDown::new(MeltType::Unauthorized, "Account is inactive"));
    }

    cata_log!(Debug, format!("Authenticated user ID: {}", session.user_id));
    Ok(SessionContext::new(session.id, session.user_id, user.role))
}

pub async fn session_auth_middleware(mut request: Request, next: Next) -> Result<Response, MeltDown> {
    let raw_token = extract_token(&request).ok_or_else(|| {
        cata_log!(Debug, "Missing session token (no Authorization header, no cb_session cookie)");
        MeltDown::session_missing()
    })?;

    let ctx = authenticate(&raw_token).await?;
    request.extensions_mut().insert(ctx);

    Ok(next.run(request).await)
}

pub async fn admin_auth_middleware(mut request: Request, next: Next) -> Result<Response, MeltDown> {
    let raw_token = extract_token(&request).ok_or_else(|| {
        cata_log!(Debug, "Missing session token for admin route");
        MeltDown::session_missing()
    })?;

    let ctx = authenticate(&raw_token).await?;

    if !ctx.is_admin() {
        cata_log!(Warning, format!("Non-admin user attempted admin access (user_id: {})", ctx.user_id));
        return Err(MeltDown::new(MeltType::Forbidden, "Admin access required"));
    }

    cata_log!(Debug, format!("Authenticated admin user (user_id: {})", ctx.user_id));
    request.extensions_mut().insert(ctx);

    Ok(next.run(request).await)
}
