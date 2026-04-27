use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    Ctx,
};

pub use crate::structs::auth::{LoginInput, LoginOutput};

pub async fn run(ctx: &Ctx, input: LoginInput) -> Result<LoginOutput, MeltDown> {
    Crank::none()
        .run(|| routines::custom::auth::login::run(ctx, LoginInput {
            email: input.email.clone(),
            password: input.password.clone(),
        }))
        .await
}
