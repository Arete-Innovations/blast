use crate::{
    cata_log,
    meltdown::*,
    models::auth::users,
    services::crypto,
    structs::UserPublic,
    Ctx,
};

#[derive(Clone)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
}

pub async fn run(ctx: &Ctx, input: RegisterInput) -> Result<UserPublic, MeltDown> {
    if input.email.trim().is_empty() {
        return Err(MeltDown::validation_failed("email is required"));
    }
    if input.password.len() < 8 {
        return Err(MeltDown::validation_failed("password must be at least 8 characters"));
    }

    let mut conn = ctx.conn().await?;

    if users::find_by_email(&mut conn, &input.email).await?.is_some() {
        return Err(MeltDown::validation_failed("email already registered"));
    }

    let hash = crypto::hash_password(&input.password)?;
    let user = users::insert_new(&mut conn, &input.email, &hash).await?;

    cata_log!(Info, format!("Registered user id={} email={}", user.id, user.email));
    Ok(UserPublic::from(user))
}
