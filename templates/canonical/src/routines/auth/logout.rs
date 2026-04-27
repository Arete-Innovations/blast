use crate::{cata_log, meltdown::*, models::auth::sessions, structs::auth::SessionContext, Ctx};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<(), MeltDown> {
    let mut conn = ctx.conn().await?;
    sessions::delete_by_token(&mut conn, &session.token).await?;
    cata_log!(Info, format!("Revoked session for user id={}", session.user_id));
    Ok(())
}
