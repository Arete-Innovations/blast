pub use crate::structs::auth::{RegisterInput, RegisterOutput};
use crate::{crank::Crank, meltdown::*, routines, Ctx};

pub async fn run(ctx: &Ctx, input: RegisterInput) -> Result<RegisterOutput, MeltDown> {
    Crank::none()
        .run(|| {
            routines::auth::register::run(
                ctx,
                RegisterInput {
                    email: input.email.clone(),
                    password: input.password.clone(),
                },
            )
        })
        .await
}
