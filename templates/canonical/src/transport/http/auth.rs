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
    structs::auth::{LoginBody, RegisterBody, SessionContext},
    transport::http::middleware::rate_limit::with_auth_rate_limit,
    Ctx,
};

pub const SESSION_COOKIE: &str = "blast_session";

fn build_session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token)).http_only(true).same_site(SameSite::Lax).path("/").build()
}

async fn register_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>, Json(body): Json<RegisterBody>) -> Result<(CookieJar, Json<SessionContext>), MeltDown> {
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
    Ok((updated, Json(output.session)))
}

async fn login_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>, Json(body): Json<LoginBody>) -> Result<(CookieJar, Json<SessionContext>), MeltDown> {
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
    Ok((updated, Json(output.session)))
}

async fn logout_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>) -> Result<(CookieJar, StatusCode), MeltDown> {
    let session = ctx.require_session()?;
    auth::logout::run(&ctx, session).await?;
    let updated = cookies.remove(Cookie::from(SESSION_COOKIE));
    Ok((updated, StatusCode::NO_CONTENT))
}

async fn me_handler(Extension(ctx): Extension<Ctx>) -> Result<Json<SessionContext>, MeltDown> {
    let session = ctx.require_session()?;
    Ok(Json(session.clone()))
}

pub fn router() -> Router<Ctx> {
    let throttled = with_auth_rate_limit(
        Router::new()
            .route("/auth/register", post(register_handler))
            .route("/auth/login", post(login_handler)),
    );
    throttled
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
}
