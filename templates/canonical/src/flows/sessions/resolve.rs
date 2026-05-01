use std::time::Duration;

use crate::{crank::Crank, meltdown::*, routines, structs::auth::SessionContext, Ctx};

pub async fn run(ctx: &Ctx, token: &str) -> Result<SessionContext, MeltDown> {
    Crank::backoff(2, Duration::from_millis(50)).run(|| routines::sessions::resolve::run(ctx, token)).await
}
