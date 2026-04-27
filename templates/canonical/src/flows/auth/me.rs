use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    structs::{auth::SessionContext, UserPublic},
    Ctx,
};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<UserPublic, MeltDown> {
    Crank::none().run(|| routines::auth::me::run(ctx, session)).await
}
