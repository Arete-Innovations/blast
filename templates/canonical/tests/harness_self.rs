mod common;

use canonical::meltdown::MeltDown;
use common::harness::TestPool;
use diesel::sql_query;
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection, RunQueryDsl,
};

#[derive(diesel::QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    n: i64,
}

#[tokio::test]
async fn rollback_discards_changes_even_on_ok() {
    let url = match std::env::var("DATABASE_URL_TEST") {
        Ok(u) => u,
        Err(_) => return,
    };

    let cfg = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&url);
    let pool: TestPool = Pool::builder(cfg).max_size(2).build().expect("pool build");

    let probe = format!("catalyst_rollback_probe_{}", uuid::Uuid::new_v4().simple());
    let probe_for_closure = probe.clone();

    let outcome = common::harness::with_test_transaction(&pool, move |conn| {
        let probe = probe_for_closure;
        Box::pin(async move {
            let create_sql = format!("CREATE TABLE {} (i int)", probe);
            sql_query(create_sql).execute(conn).await.map_err(MeltDown::from)?;
            Ok::<(), MeltDown>(())
        })
    })
    .await;

    assert!(outcome.is_ok(), "wrapper returned Err: {:?}", outcome);

    let mut verify_conn = pool.get().await.expect("verify conn");
    let hits: Vec<CountRow> = diesel::sql_query("SELECT count(*)::bigint AS n FROM pg_class WHERE relname = $1 AND relkind = 'r'")
        .bind::<diesel::sql_types::Text, _>(&probe)
        .load(&mut *verify_conn)
        .await
        .expect("verify load");
    assert_eq!(hits[0].n, 0, "rollback did not discard CREATE TABLE");
}
