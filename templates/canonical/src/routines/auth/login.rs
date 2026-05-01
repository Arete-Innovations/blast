use crate::{
    cata_log,
    meltdown::*,
    models::auth::{sessions, users},
    services::{crypto, time},
    structs::{
        auth::{LoginInput, LoginOutput, SessionContext, SESSION_TTL_SECS},
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
    let expires_at = time::now_unix() + SESSION_TTL_SECS;
    let session_row = sessions::insert_session(&mut conn, user.id, &token, expires_at).await?;

    cata_log!(Info, format!("Issued session for user id={}", user.id));
    let session_ctx = SessionContext::new(session_row.id, user.id, user.role, &token);
    Ok(LoginOutput {
        token,
        user: UserPublic::from(user),
        session: session_ctx,
    })
}
