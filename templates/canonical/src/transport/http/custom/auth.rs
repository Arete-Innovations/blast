use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    middleware::from_fn,
    routing::{get, post},
    Router,
};
use base64::Engine as _;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::{
    cata_log,
    database::db::establish_connection,
    meltdown::*,
    models::auth::{sessions, users},
    structs::{auth::SessionContext, UserPublic},
    transport::http::middleware::auth::session_auth_middleware,
    Ctx,
};

const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 7;

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserPublic,
}

fn hash_password(plain: &str) -> Result<String, MeltDown> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let phc = argon
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| MeltDown::new(MeltType::Unexpected("argon2_hash".into()), format!("argon2 hash: {e}")))?;
    Ok(phc.to_string())
}

fn verify_password(plain: &str, phc: &str) -> Result<bool, MeltDown> {
    let parsed = PasswordHash::new(phc)
        .map_err(|e| MeltDown::new(MeltType::Unexpected("argon2_parse".into()), format!("argon2 parse: {e}")))?;
    let argon = Argon2::default();
    Ok(argon.verify_password(plain.as_bytes(), &parsed).is_ok())
}

fn mint_session_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn register_handler(Json(body): Json<RegisterInput>) -> Result<Json<UserPublic>, MeltDown> {
    cata_log!(Info, format!("Register attempt for email: {}", body.email));

    if body.email.trim().is_empty() {
        return Err(MeltDown::validation_failed("email is required"));
    }
    if body.password.len() < 8 {
        return Err(MeltDown::validation_failed("password must be at least 8 characters"));
    }

    let mut conn = establish_connection().await?;

    if users::find_by_email(&mut conn, &body.email).await?.is_some() {
        return Err(MeltDown::validation_failed("email already registered"));
    }

    let hash = hash_password(&body.password)?;
    let user = users::insert_new(&mut conn, &body.email, &hash).await?;

    cata_log!(Info, format!("Registered user id={} email={}", user.id, user.email));
    Ok(Json(UserPublic::from(user)))
}

async fn login_handler(Json(body): Json<LoginInput>) -> Result<Json<LoginResponse>, MeltDown> {
    cata_log!(Info, format!("Login attempt for email: {}", body.email));

    let mut conn = establish_connection().await?;
    let user = users::find_by_email(&mut conn, &body.email)
        .await?
        .ok_or_else(MeltDown::auth_rejected)?;

    if !verify_password(&body.password, &user.password_hash)? {
        cata_log!(Warning, format!("Invalid password for email: {}", body.email));
        return Err(MeltDown::auth_rejected());
    }

    let token = mint_session_token();
    let expires_at = now_unix() + SESSION_TTL_SECS;
    let _session = sessions::insert_session(&mut conn, user.id, &token, expires_at).await?;

    cata_log!(Info, format!("Issued session for user id={}", user.id));
    Ok(Json(LoginResponse {
        token,
        user: UserPublic::from(user),
    }))
}

async fn logout_handler(
    Extension(ctx): Extension<SessionContext>,
) -> Result<StatusCode, MeltDown> {
    let mut conn = establish_connection().await?;
    sessions::delete_by_token(&mut conn, &ctx.token).await?;
    cata_log!(Info, format!("Revoked session for user id={}", ctx.user_id));
    Ok(StatusCode::NO_CONTENT)
}

async fn me_handler(
    Extension(ctx): Extension<SessionContext>,
) -> Result<Json<UserPublic>, MeltDown> {
    let mut conn = establish_connection().await?;
    let user = users::find_by_id(&mut conn, ctx.user_id)
        .await?
        .ok_or_else(|| MeltDown::session_invalid("Session user no longer exists"))?;
    Ok(Json(UserPublic::from(user)))
}

pub fn router() -> Router<Ctx> {
    let public = Router::new()
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler));

    let protected = Router::new()
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
        .layer(from_fn(session_auth_middleware));

    public.merge(protected)
}
