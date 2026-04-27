use crate::{
    meltdown::*,
    models::auth::users,
    structs::{auth::SessionContext, UserPublic},
    Ctx,
};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<UserPublic, MeltDown> {
    let mut conn = ctx.conn().await?;
    let user = users::find_by_id(&mut conn, session.user_id)
        .await?
        .ok_or_else(|| MeltDown::session_invalid("Session user no longer exists"))?;
    Ok(UserPublic::from(user))
}
