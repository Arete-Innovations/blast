use crate::{
    crank::Crank,
    meltdown::*,
    routines,
    structs::UserPublic,
    Ctx,
};

pub use routines::custom::auth::register::RegisterInput;

pub async fn run(ctx: &Ctx, input: RegisterInput) -> Result<UserPublic, MeltDown> {
    Crank::none()
        .run(|| routines::custom::auth::register::run(ctx, RegisterInput {
            email: input.email.clone(),
            password: input.password.clone(),
        }))
        .await
}

