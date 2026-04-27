use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    structs::auth::SessionContext,
    Ctx,
};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<(), MeltDown> {
    Crank::none()
        .run(|| routines::custom::auth::logout::run(ctx, session))
        .await
}
