use crate::{
    cata_log,
    meltdown::*,
    models::auth::{sessions, users},
    services::{crypto, time},
    structs::{
        auth::{RegisterInput, RegisterOutput, SessionContext, SESSION_TTL_SECS},
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
    let expires_at = time::now_unix() + SESSION_TTL_SECS;
    let session_row = sessions::insert_session(&mut conn, user.id, &token, expires_at).await?;

    cata_log!(Info, format!("Registered user id={} email={}", user.id, user.email));
    let session_ctx = SessionContext::new(session_row.id, user.id, user.role, &token);
    Ok(RegisterOutput {
        token,
        user: UserPublic::from(user),
        session: session_ctx,
    })
}
