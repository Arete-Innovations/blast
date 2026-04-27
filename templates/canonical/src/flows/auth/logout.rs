use crate::{crank::Crank, meltdown::*, routines, structs::auth::SessionContext, Ctx};

pub async fn run(ctx: &Ctx, session: &SessionContext) -> Result<(), MeltDown> {
    Crank::none().run(|| routines::auth::logout::run(ctx, session)).await
}
