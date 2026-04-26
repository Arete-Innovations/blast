use axum::{
    extract::{Extension, Json},
    http::{header::SET_COOKIE, HeaderValue},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use axum_extra::extract::CookieJar;
use chrono::{TimeZone, Utc};
use diesel::sql_types::{Bool, Integer, Nullable, Text, Varchar};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};

use crate::{
    cata_log,
    database::db::establish_connection,
    meltdown::*,
    middleware::auth_middleware::{SessionContext, SESSION_COOKIE},
    models::sessions,
};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub token: String,
    pub expires_at: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub role: String,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
}

/// Minimal user row for login — fetched without depending on a concrete Users struct.
#[derive(diesel::QueryableByName, Debug)]
struct LoginUserRow {
    #[diesel(sql_type = Integer)]
    id: i32,
    #[diesel(sql_type = Text)]
    username: String,
    #[diesel(sql_type = Nullable<Text>)]
    email: Option<String>,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Bool)]
    active: bool,
    #[diesel(sql_type = Varchar)]
    password_hash: String,
}

fn secure_cookie() -> bool {
    !matches!(
        std::env::var("SESSION_COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()).to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn build_session_cookie(token: &str, max_age_seconds: i64) -> String {
    let flags = if secure_cookie() {
        "HttpOnly; Secure; SameSite=Lax"
    } else {
        "HttpOnly; SameSite=Lax"
    };
    format!(
        "{name}={value}; {flags}; Path=/; Max-Age={max_age}",
        name = SESSION_COOKIE,
        value = token,
        flags = flags,
        max_age = max_age_seconds.max(0),
    )
}

fn build_cleared_cookie() -> String {
    let flags = if secure_cookie() {
        "HttpOnly; Secure; SameSite=Lax"
    } else {
        "HttpOnly; SameSite=Lax"
    };
    format!("{name}=; {flags}; Path=/; Max-Age=0", name = SESSION_COOKIE, flags = flags)
}

async fn login(jar: CookieJar, Json(request): Json<LoginRequest>) -> Result<Response, MeltDown> {

    cata_log!(Info, format!("Login attempt for username: {}", request.username));

    let mut conn = establish_connection().await?;
    let rows: Vec<LoginUserRow> = diesel::sql_query(
        "SELECT id, username, email, role, active, password_hash FROM users WHERE username = $1 AND active = true LIMIT 1"
    )
    .bind::<Text, _>(&request.username)
    .load(&mut conn)
    .await
    .map_err(|e| MeltDown::from(e).with_context("operation", "login_fetch_user"))?;

    let user = rows.into_iter().next().ok_or_else(|| {
        cata_log!(Warning, format!("Login failed: no active user with username {}", request.username));
        MeltDown::auth_rejected()
    })?;

    let password_hash = user.password_hash.clone();
    let password = request.password.clone();
    let password_ok = tokio::task::spawn_blocking(move || {
        bcrypt::verify(&password, &password_hash).unwrap_or(false)
    })
    .await
    .map_err(|e| MeltDown::new(MeltType::Unexpected("tokio_task_join".to_string()), format!("Task join error: {}", e)))?;

    if !password_ok {
        cata_log!(Warning, format!("Invalid password for user: {}", request.username));
        return Err(MeltDown::auth_rejected());
    }

    let (session, raw_token) = sessions::create_session(user.id, None, None).await?;

    cata_log!(Info, format!("Issued session {} for user {} (ID: {})", session.id, user.username, user.id));

    let max_age = (session.expires_at - Utc::now().timestamp()).max(0);
    let cookie = build_session_cookie(&raw_token, max_age);

    let expires_at_iso = Utc
        .timestamp_opt(session.expires_at, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default();

    let body = LoginResponse {
        success: true,
        message: "Login successful".to_string(),
        token: raw_token,
        expires_at: expires_at_iso,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email.unwrap_or_default(),
            role: user.role,
            active: user.active,
        },
    };

    let _ = jar;
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, HeaderValue::from_str(&cookie).map_err(|e| MeltDown::new(MeltType::Unexpected("set_cookie".into()), format!("invalid Set-Cookie header: {}", e)))?);

    Ok(response)
}

async fn logout(ctx: Option<Extension<SessionContext>>) -> Result<Response, MeltDown> {
    cata_log!(Debug, "Logout request");

    if let Some(Extension(ctx)) = ctx {
        if let Err(e) = sessions::revoke(ctx.session_id).await {
            cata_log!(Warning, format!("Failed to revoke session {}: {}", ctx.session_id, e.log_message()));
        }
    }

    let clear_cookie = build_cleared_cookie();

    let body = AuthResponse {
        success: true,
        message: "Logged out successfully".to_string(),
    };

    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, HeaderValue::from_str(&clear_cookie).map_err(|e| MeltDown::new(MeltType::Unexpected("set_cookie".into()), format!("invalid Set-Cookie header: {}", e)))?);

    Ok(response)
}

async fn me(Extension(ctx): Extension<SessionContext>) -> Result<Json<UserInfo>, MeltDown> {

    let mut conn = establish_connection().await?;
    let rows: Vec<LoginUserRow> = diesel::sql_query(
        "SELECT id, username, email, role, active, password_hash FROM users WHERE id = $1 LIMIT 1"
    )
    .bind::<Integer, _>(ctx.user_id)
    .load(&mut conn)
    .await
    .map_err(|e| MeltDown::from(e).with_context("operation", "me_fetch_user"))?;

    let user = rows.into_iter().next().ok_or_else(|| MeltDown::session_invalid("User not found"))?;

    Ok(Json(UserInfo {
        id: user.id,
        username: user.username,
        email: user.email.unwrap_or_default(),
        role: user.role,
        active: user.active,
    }))
}

pub fn routes() -> Router {
    use axum::middleware::from_fn;

    use crate::middleware::auth_middleware::session_auth_middleware;

    let public = Router::new().route("/auth/login", post(login));

    let protected = Router::new()
        .route("/auth/logout", post(logout))
        .route("/auth/me", post(me))
        .layer(from_fn(session_auth_middleware));

    public.merge(protected)
}
