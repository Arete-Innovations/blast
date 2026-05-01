use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use crate::{
    cata_log,
    flows::auth,
    meltdown::*,
    structs::{
        auth::{AuthResponse, LoginBody, RegisterBody},
        UserPublic,
    },
    Ctx,
};

pub const SESSION_COOKIE: &str = "blast_session";

fn build_session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token)).http_only(true).secure(true).same_site(SameSite::Strict).path("/").build()
}

async fn register_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>, Json(body): Json<RegisterBody>) -> Result<(CookieJar, Json<AuthResponse>), MeltDown> {
    cata_log!(Info, format!("Register attempt for email: {}", body.email));
    let output = auth::register::run(
        &ctx,
        auth::register::RegisterInput {
            email: body.email,
            password: body.password,
        },
    )
    .await?;
    let updated = cookies.add(build_session_cookie(output.token.clone()));
    Ok((updated, Json(AuthResponse { token: output.token, user: output.user })))
}

async fn login_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>, Json(body): Json<LoginBody>) -> Result<(CookieJar, Json<AuthResponse>), MeltDown> {
    cata_log!(Info, format!("Login attempt for email: {}", body.email));
    let output = auth::login::run(
        &ctx,
        auth::login::LoginInput {
            email: body.email,
            password: body.password,
        },
    )
    .await?;
    let updated = cookies.add(build_session_cookie(output.token.clone()));
    Ok((updated, Json(AuthResponse { token: output.token, user: output.user })))
}

async fn logout_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>) -> Result<(CookieJar, StatusCode), MeltDown> {
    let session = ctx.require_session()?;
    auth::logout::run(&ctx, session).await?;
    let updated = cookies.remove(Cookie::from(SESSION_COOKIE));
    Ok((updated, StatusCode::NO_CONTENT))
}

async fn me_handler(Extension(ctx): Extension<Ctx>) -> Result<Json<UserPublic>, MeltDown> {
    let session = ctx.require_session()?;
    let user = auth::me::run(&ctx, session).await?;
    Ok(Json(user))
}

pub fn router() -> Router<Ctx> {
    Router::new()
        .route("/auth/register", post(register_handler))
        .route("/auth/login", post(login_handler))
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
}
