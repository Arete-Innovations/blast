use crate::{
    cata_log,
    meltdown::*,
    models::auth::{sessions, users},
    services::crypto,
    structs::{auth::SessionContext, UserPublic},
    Ctx,
};

pub const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 7;

#[derive(Clone)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

pub struct LoginOutput {
    pub token: String,
    pub user: UserPublic,
    pub session: SessionContext,
}

pub async fn run(ctx: &Ctx, input: LoginInput) -> Result<LoginOutput, MeltDown> {
    let mut conn = ctx.conn().await?;
    let user = users::find_by_email(&mut conn, &input.email)
        .await?
        .ok_or_else(MeltDown::auth_rejected)?;

    if !crypto::verify_password(&input.password, &user.password_hash)? {
        cata_log!(Warning, format!("Invalid password for email: {}", input.email));
        return Err(MeltDown::auth_rejected());
    }

    let token = crypto::mint_session_token();
    let expires_at = crypto::now_unix() + SESSION_TTL_SECS;
    let session_row = sessions::insert_session(&mut conn, user.id, &token, expires_at).await?;

    cata_log!(Info, format!("Issued session for user id={}", user.id));
    let session_ctx = SessionContext::new(session_row.id, user.id, user.role, &token);
    Ok(LoginOutput {
        token,
        user: UserPublic::from(user),
        session: session_ctx,
    })
}
