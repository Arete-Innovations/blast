use diesel::sql_query;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    scoped_futures::ScopedFutureExt,
    AsyncPgConnection, RunQueryDsl,
};

use canonical::{
    ctx::{Ctx, CtxPool},
    meltdown::MeltDown,
};

#[derive(diesel::QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    n: i64,
}

fn build_pool(url: &str) -> CtxPool {
    let cfg = AsyncDieselConnectionManager::<AsyncPgConnection>::new(url);
    Pool::builder(cfg).max_size(2).build().expect("pool build")
}

#[tokio::test]
async fn transaction_returns_ok_value() {
    let url = match std::env::var("DATABASE_URL_TEST") {
        Ok(u) => u,
        Err(_) => return,
    };
    let ctx = Ctx::anonymous(build_pool(&url));

    let outcome: Result<i64, MeltDown> = ctx
        .transaction(|conn| {
            async move {
                let rows: Vec<CountRow> = sql_query("SELECT 7::bigint AS n")
                    .load(conn)
                    .await
                    .map_err(MeltDown::from)?;
                Ok(rows[0].n)
            }
            .scope_boxed()
        })
        .await;

    assert_eq!(outcome.expect("transaction ok"), 7);
}

#[tokio::test]
async fn transaction_rolls_back_on_err() {
    let url = match std::env::var("DATABASE_URL_TEST") {
        Ok(u) => u,
        Err(_) => return,
    };
    let ctx = Ctx::anonymous(build_pool(&url));

    let probe = format!("catalyst_ctx_tx_probe_{}", uuid::Uuid::new_v4().simple());
    let probe_for_closure = probe.clone();

    let outcome: Result<(), MeltDown> = ctx
        .transaction(move |conn| {
            let probe = probe_for_closure;
            async move {
                let create_sql = format!("CREATE TABLE {} (i int)", probe);
                sql_query(create_sql)
                    .execute(conn)
                    .await
                    .map_err(MeltDown::from)?;
                Err(MeltDown::bad_request("intentional rollback"))
            }
            .scope_boxed()
        })
        .await;

    assert!(outcome.is_err(), "expected Err, got {:?}", outcome.as_ref().err());

    let mut verify_conn = ctx.conn().await.expect("verify conn");
    let hits: Vec<CountRow> = sql_query(
        "SELECT count(*)::bigint AS n FROM pg_class WHERE relname = $1 AND relkind = 'r'",
    )
    .bind::<diesel::sql_types::Text, _>(&probe)
    .load(&mut *verify_conn)
    .await
    .expect("verify load");
    assert_eq!(hits[0].n, 0, "rollback did not discard CREATE TABLE");
}
