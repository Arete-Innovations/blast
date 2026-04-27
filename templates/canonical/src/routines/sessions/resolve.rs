use crate::{
    meltdown::*,
    models,
    structs::auth::SessionContext,
    Ctx,
};

pub async fn run(ctx: &Ctx, token: &str) -> Result<SessionContext, MeltDown> {
    let mut conn = ctx.conn().await?;
    let (session, user) = models::auth::sessions::find_by_token(&mut conn, token)
        .await?
        .ok_or_else(|| MeltDown::session_invalid("Session token not recognised or expired"))?;
    Ok(SessionContext::new(session.id, session.user_id, user.role, token))
}
