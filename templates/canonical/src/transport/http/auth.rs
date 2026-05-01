use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    routing::{get, post},
    Router,
};

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

async fn register_handler(Extension(ctx): Extension<Ctx>, Json(body): Json<RegisterBody>) -> Result<Json<AuthResponse>, MeltDown> {
    cata_log!(Info, format!("Register attempt for email: {}", body.email));
    let output = auth::register::run(
        &ctx,
        auth::register::RegisterInput {
            email: body.email,
            password: body.password,
        },
    )
    .await?;
    Ok(Json(AuthResponse { token: output.token, user: output.user }))
}

async fn login_handler(Extension(ctx): Extension<Ctx>, Json(body): Json<LoginBody>) -> Result<Json<AuthResponse>, MeltDown> {
    cata_log!(Info, format!("Login attempt for email: {}", body.email));
    let output = auth::login::run(
        &ctx,
        auth::login::LoginInput {
            email: body.email,
            password: body.password,
        },
    )
    .await?;
    Ok(Json(AuthResponse { token: output.token, user: output.user }))
}

async fn logout_handler(Extension(ctx): Extension<Ctx>) -> Result<StatusCode, MeltDown> {
    let session = ctx.require_session()?;
    auth::logout::run(&ctx, session).await?;
    Ok(StatusCode::NO_CONTENT)
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
