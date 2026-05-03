//! Byte-stable file content templates for the auth emitter.
//!
//! Each `pub const` carries the exact body that will be written to the
//! corresponding generated file (sans the codegen marker — the runner
//! prepends the marker). These bodies were lifted verbatim from the
//! reference catalyst tree post the auth-into-generated reshuffle.
//!
//! Convention: keep each template byte-identical to the reference file
//! at `catalyst/src/...`. Drift between this template and the reference
//! is a bug — the reference IS the spec.

// ── structs/generated ─────────────────────────────────────────────────────

pub const STRUCTS_USERS: &str = r#"#[cfg(not(target_arch = "wasm32"))]
use diesel::{prelude::*, Queryable};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::database::schema::users;
use crate::structs::generated::UserRole;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Queryable, QueryableByName, Selectable, Debug, Clone, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: i64,
    pub email: String,
    pub role: UserRole,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<&User> for UserPublic {
    fn from(u: &User) -> Self {
        Self {
            id: u.id,
            email: u.email.clone(),
            role: u.role.clone(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self { id: u.id, email: u.email, role: u.role }
    }
}
"#;

pub const STRUCTS_AUTH_LOGIN: &str = r#"use serde::{Deserialize, Serialize};

use crate::structs::{vendored::auth::SessionContext, UserPublic};

#[derive(Clone)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

pub struct LoginOutput {
    pub token: String,
    pub user: UserPublic,
    pub session: SessionContext,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}
"#;

pub const STRUCTS_AUTH_REGISTER: &str = r#"use serde::Deserialize;

use crate::structs::{vendored::auth::SessionContext, UserPublic};

#[derive(Clone)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
}

pub struct RegisterOutput {
    pub token: String,
    pub user: UserPublic,
    pub session: SessionContext,
}
"#;

pub const STRUCTS_AUTH_MOD: &str = r#"pub mod login;
pub mod register;

pub use login::{AuthResponse, LoginBody, LoginInput, LoginOutput};
pub use register::{RegisterBody, RegisterInput, RegisterOutput};
"#;

// ── models/generated ──────────────────────────────────────────────────────

pub const MODELS_USERS: &str = r#"use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    database::schema::users::dsl as users_dsl,
    meltdown::*,
    structs::{generated::UserRole, NewUser, User},
};

pub async fn find_by_email(conn: &mut AsyncPgConnection, email: &str) -> Result<Option<User>, MeltDown> {
    users_dsl::users
        .filter(users_dsl::email.eq(email))
        .filter(users_dsl::deleted_at.is_null())
        .select(User::as_select())
        .first::<User>(conn)
        .await
        .optional()
        .map_err(|e| MeltDown::from(e).with_context("operation", "find_user_by_email"))
}

pub async fn find_by_id(conn: &mut AsyncPgConnection, id: i64) -> Result<Option<User>, MeltDown> {
    users_dsl::users
        .filter(users_dsl::id.eq(id))
        .filter(users_dsl::deleted_at.is_null())
        .select(User::as_select())
        .first::<User>(conn)
        .await
        .optional()
        .map_err(|e| MeltDown::from(e).with_context("operation", "find_user_by_id"))
}

pub async fn insert_new(conn: &mut AsyncPgConnection, email: &str, password_hash: &str) -> Result<User, MeltDown> {
    let new_user = NewUser {
        email: email.to_string(),
        password_hash: password_hash.to_string(),
    };

    diesel::insert_into(users_dsl::users)
        .values(&new_user)
        .returning(User::as_select())
        .get_result::<User>(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "insert_new_user"))
}

pub async fn set_role(conn: &mut AsyncPgConnection, id: i64, role: UserRole) -> Result<User, MeltDown> {
    diesel::update(users_dsl::users.filter(users_dsl::id.eq(id)))
        .set(users_dsl::role.eq(role))
        .returning(User::as_select())
        .get_result::<User>(conn)
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "set_user_role"))
}
"#;

// ── routines/generated/auth ───────────────────────────────────────────────

pub const ROUTINES_AUTH_LOGIN: &str = r#"use crate::{
    cata_log,
    config::cfg,
    meltdown::*,
    models::{vendored::auth::sessions, generated::users},
    services::vendored::{crypto, time},
    structs::{
        vendored::auth::SessionContext,
        generated::auth::{LoginInput, LoginOutput},
        UserPublic,
    },
    Ctx,
};

pub async fn run(ctx: &Ctx, input: LoginInput) -> Result<LoginOutput, MeltDown> {
    let email = input.email.trim().to_lowercase();
    let mut conn = ctx.conn().await?;
    let user = users::find_by_email(&mut conn, &email).await?.ok_or_else(MeltDown::auth_rejected)?;

    if !crypto::verify_password(&input.password, &user.password_hash)? {
        cata_log!(Warning, format!("Invalid password for email: {}", email));
        return Err(MeltDown::auth_rejected());
    }

    let token = crypto::mint_session_token();
    let expires_at = time::now_unix() + cfg().auth.session_ttl_secs;
    let session_row = sessions::insert_session(&mut conn, user.id, &token, expires_at).await?;

    cata_log!(Info, format!("Issued session for user id={}", user.id));
    let session_ctx = SessionContext::new(session_row.id, user.id, user.role, &token);
    Ok(LoginOutput {
        token,
        user: UserPublic::from(user),
        session: session_ctx,
    })
}
"#;

pub const ROUTINES_AUTH_REGISTER: &str = r#"use crate::{
    cata_log,
    config::cfg,
    meltdown::*,
    models::{vendored::auth::sessions, generated::users},
    services::vendored::{crypto, time},
    structs::{
        vendored::auth::SessionContext,
        generated::auth::{RegisterInput, RegisterOutput},
        UserPublic,
    },
    Ctx,
};

pub async fn run(ctx: &Ctx, input: RegisterInput) -> Result<RegisterOutput, MeltDown> {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(MeltDown::validation_failed("email is required"));
    }
    if input.password.len() < 8 {
        return Err(MeltDown::validation_failed("password must be at least 8 characters"));
    }

    let mut conn = ctx.conn().await?;

    if users::find_by_email(&mut conn, &email).await?.is_some() {
        return Err(MeltDown::validation_failed("email already registered"));
    }

    let hash = crypto::hash_password(&input.password)?;
    let user = users::insert_new(&mut conn, &email, &hash).await?;

    let token = crypto::mint_session_token();
    let expires_at = time::now_unix() + cfg().auth.session_ttl_secs;
    let session_row = sessions::insert_session(&mut conn, user.id, &token, expires_at).await?;

    cata_log!(Info, format!("Registered user id={} email={}", user.id, user.email));
    let session_ctx = SessionContext::new(session_row.id, user.id, user.role, &token);
    Ok(RegisterOutput {
        token,
        user: UserPublic::from(user),
        session: session_ctx,
    })
}
"#;

pub const ROUTINES_AUTH_LOGOUT: &str = r#"use crate::{cata_log, meltdown::*, models::vendored::auth::sessions, structs::vendored::auth::SessionContext, Ctx};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<(), MeltDown> {
    let mut conn = ctx.conn().await?;
    sessions::delete_by_token(&mut conn, &session.token).await?;
    cata_log!(Info, format!("Revoked session for user id={}", session.user_id));
    Ok(())
}
"#;

pub const ROUTINES_AUTH_ME: &str = r#"use crate::{
    meltdown::*,
    models::generated::users,
    structs::{vendored::auth::SessionContext, UserPublic},
    Ctx,
};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<UserPublic, MeltDown> {
    let mut conn = ctx.conn().await?;
    let user = users::find_by_id(&mut conn, session.user_id).await?.ok_or_else(|| MeltDown::session_invalid("Session user no longer exists"))?;
    Ok(UserPublic::from(user))
}
"#;

pub const ROUTINES_AUTH_MOD: &str = r#"pub mod login;
pub mod logout;
pub mod me;
pub mod register;
"#;

// ── flows/generated/auth ──────────────────────────────────────────────────

pub const FLOWS_AUTH_LOGIN: &str = r#"pub use crate::structs::generated::auth::{LoginInput, LoginOutput};
use crate::{crank::Crank, meltdown::*, routines, Ctx};

pub async fn run(ctx: &Ctx, input: LoginInput) -> Result<LoginOutput, MeltDown> {
    Crank::none()
        .run(|| {
            routines::generated::auth::login::run(
                ctx,
                LoginInput {
                    email: input.email.clone(),
                    password: input.password.clone(),
                },
            )
        })
        .await
}
"#;

pub const FLOWS_AUTH_REGISTER: &str = r#"pub use crate::structs::generated::auth::{RegisterInput, RegisterOutput};
use crate::{crank::Crank, meltdown::*, routines, Ctx};

pub async fn run(ctx: &Ctx, input: RegisterInput) -> Result<RegisterOutput, MeltDown> {
    Crank::none()
        .run(|| {
            routines::generated::auth::register::run(
                ctx,
                RegisterInput {
                    email: input.email.clone(),
                    password: input.password.clone(),
                },
            )
        })
        .await
}
"#;

pub const FLOWS_AUTH_LOGOUT: &str = r#"use crate::{crank::Crank, meltdown::*, routines, structs::vendored::auth::SessionContext, Ctx};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<(), MeltDown> {
    Crank::none().run(|| routines::generated::auth::logout::run(ctx, session)).await
}
"#;

pub const FLOWS_AUTH_ME: &str = r#"use std::time::Duration;

use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    structs::{vendored::auth::SessionContext, UserPublic},
    Ctx,
};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<UserPublic, MeltDown> {
    Crank::backoff(2, Duration::from_millis(50)).run(|| routines::generated::auth::me::run(ctx, session)).await
}
"#;

pub const FLOWS_AUTH_MOD: &str = r#"pub mod login;
pub mod logout;
pub mod me;
pub mod register;
"#;

// ── transport/http/generated ──────────────────────────────────────────────

pub const HTTP_AUTH: &str = r#"use axum::{
    extract::{rejection::JsonRejection, Extension, Json},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use crate::{
    cata_log,
    config::cfg,
    flows::generated::auth,
    meltdown::*,
    structs::{
        vendored::auth::SessionContext,
        generated::auth::{LoginBody, RegisterBody},
    },
    transport::http::vendored::middleware::rate_limit::with_auth_rate_limit,
    Ctx,
};

fn build_session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((cfg().auth.session_cookie_name.clone(), token)).http_only(true).same_site(SameSite::Lax).path("/").build()
}

async fn register_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>, body: Result<Json<RegisterBody>, JsonRejection>) -> Result<(CookieJar, Json<SessionContext>), MeltDown> {
    let Json(body) = body?;
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

async fn login_handler(cookies: CookieJar, Extension(ctx): Extension<Ctx>, body: Result<Json<LoginBody>, JsonRejection>) -> Result<(CookieJar, Json<SessionContext>), MeltDown> {
    let Json(body) = body?;
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
    let updated = cookies.remove(Cookie::from(cfg().auth.session_cookie_name.clone()));
    Ok((updated, StatusCode::NO_CONTENT))
}

async fn me_handler(Extension(ctx): Extension<Ctx>) -> Result<Json<SessionContext>, MeltDown> {
    let session = ctx.require_session()?;
    Ok(Json(session.clone()))
}

pub fn router() -> Router<Ctx> {
    let throttled = with_auth_rate_limit(Router::new().route("/auth/register", post(register_handler)).route("/auth/login", post(login_handler)));
    throttled.route("/auth/logout", post(logout_handler)).route("/auth/me", get(me_handler))
}
"#;

pub mod leptos_pages;
pub use leptos_pages::*;

