use axum::{
    extract::{Extension, Json, State},
    http::StatusCode,
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};

use crate::{
    cata_log,
    flows::auth,
    meltdown::*,
    structs::{
        auth::{LoginBody, LoginResponse, RegisterBody, SessionContext},
        UserPublic,
    },
    transport::http::middleware::auth::session_auth_middleware,
    Ctx,
};

async fn register_handler(State(ctx): State<Ctx>, Json(body): Json<RegisterBody>) -> Result<Json<UserPublic>, MeltDown> {
    cata_log!(Info, format!("Register attempt for email: {}", body.email));
    let user = auth::register::run(
        &ctx,
        auth::register::RegisterInput {
            email: body.email,
            password: body.password,
        },
    )
    .await?;
    Ok(Json(user))
}

async fn login_handler(State(ctx): State<Ctx>, Json(body): Json<LoginBody>) -> Result<Json<LoginResponse>, MeltDown> {
    cata_log!(Info, format!("Login attempt for email: {}", body.email));
    let output = auth::login::run(
        &ctx,
        auth::login::LoginInput {
            email: body.email,
            password: body.password,
        },
    )
    .await?;
    Ok(Json(LoginResponse { token: output.token, user: output.user }))
}

async fn logout_handler(State(ctx): State<Ctx>, Extension(session): Extension<SessionContext>) -> Result<StatusCode, MeltDown> {
    auth::logout::run(&ctx, &session).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me_handler(State(ctx): State<Ctx>, Extension(session): Extension<SessionContext>) -> Result<Json<UserPublic>, MeltDown> {
    let user = auth::me::run(&ctx, &session).await?;
    Ok(Json(user))
}

pub fn router(ctx: Ctx) -> Router<Ctx> {
    let public = Router::new().route("/auth/register", post(register_handler)).route("/auth/login", post(login_handler));

    let protected = Router::new()
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
        .layer(from_fn_with_state(ctx, session_auth_middleware));

    public.merge(protected)
}
