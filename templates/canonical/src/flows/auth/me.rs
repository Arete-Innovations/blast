use std::time::Duration;

use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    structs::{auth::SessionContext, UserPublic},
    Ctx,
};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<UserPublic, MeltDown> {
    Crank::backoff(2, Duration::from_millis(50)).run(|| routines::auth::me::run(ctx, session)).await
}
