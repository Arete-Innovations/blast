use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    structs::auth::SessionContext,
    Ctx,
};

pub async fn run(ctx: &Ctx, token: &str) -> Result<SessionContext, MeltDown> {
    Crank::none()
        .run(|| routines::custom::sessions::resolve::run(ctx, token))
        .await
}
