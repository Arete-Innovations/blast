use canonical::{
    flows::auth::{login, register},
    meltdown::MeltDown,
    structs::auth::{LoginInput, RegisterInput},
    Ctx,
};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};

fn build_pool(url: &str) -> Result<Pool<AsyncPgConnection>, MeltDown> {
    let cfg = AsyncDieselConnectionManager::<AsyncPgConnection>::new(url);
    Pool::builder(cfg)
        .max_size(2)
        .build()
        .map_err(|e| MeltDown::db_connection(format!("test pool build failed: {}", e)))
}

#[tokio::test]
async fn whitespace_uppercase_email_normalizes_round_trip() -> Result<(), MeltDown> {
    let url = match std::env::var("DATABASE_URL_TEST") {
        Ok(u) => u,
        Err(_) => return Ok(()),
    };

    let pool = build_pool(&url)?;
    let ctx = Ctx::anonymous(pool);

    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| MeltDown::bad_request(format!("clock: {}", e)))?.as_nanos();
    let dirty = format!("  Tester+{}@Example.COM  ", nonce);
    let clean = format!("tester+{}@example.com", nonce);

    let registered = register::run(
        &ctx,
        RegisterInput {
            email: dirty.clone(),
            password: "correcthorsebatterystaple".to_string(),
        },
    )
    .await?;

    assert_eq!(registered.user.email, clean, "register stored normalized email");

    let logged_in = login::run(
        &ctx,
        LoginInput {
            email: clean.clone(),
            password: "correcthorsebatterystaple".to_string(),
        },
    )
    .await?;

    assert_eq!(logged_in.user.id, registered.user.id, "login finds the same user via clean email");
    assert_eq!(logged_in.user.email, clean);

    let logged_in_dirty = login::run(
        &ctx,
        LoginInput {
            email: dirty.clone(),
            password: "correcthorsebatterystaple".to_string(),
        },
    )
    .await?;

    assert_eq!(logged_in_dirty.user.id, registered.user.id, "login normalizes whitespace+case at lookup");

    Ok(())
}
