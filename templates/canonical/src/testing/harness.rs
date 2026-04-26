
use diesel::sql_query;
use diesel_async::{
    pooled_connection::deadpool::Pool,
    scoped_futures::ScopedBoxFuture,
    AsyncPgConnection, RunQueryDsl,
};

use crate::meltdown::{MeltDown, MeltType};

pub type TestPool = Pool<AsyncPgConnection>;

pub async fn with_test_transaction<'a, T, F>(pool: &TestPool, f: F) -> Result<T, MeltDown>
where
    F: for<'r> FnOnce(&'r mut AsyncPgConnection) -> ScopedBoxFuture<'a, 'r, Result<T, MeltDown>>
        + Send
        + 'a,
    T: Send + 'a,
{
    let mut conn = pool
        .get()
        .await
        .map_err(|e| MeltDown::db_connection(format!("test pool acquire failed: {}", e)))?;

    sql_query("BEGIN")
        .execute(&mut *conn)
        .await
        .map_err(|e| MeltDown::new(MeltType::DatabaseError, format!("BEGIN failed: {}", e)))?;

    let user_result = f(&mut *conn).await;

    let rollback_result = sql_query("ROLLBACK").execute(&mut *conn).await;

    if let Err(e) = rollback_result {
        return Err(MeltDown::new(
            MeltType::DatabaseError,
            format!("ROLLBACK failed: {}", e),
        ));
    }

    user_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::prelude::*;
    use diesel_async::pooled_connection::AsyncDieselConnectionManager;

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

        let outcome = with_test_transaction(&pool, move |conn| {
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
        let hits: Vec<CountRow> = diesel::sql_query(
            "SELECT count(*)::bigint AS n FROM pg_class WHERE relname = $1 AND relkind = 'r'",
        )
        .bind::<diesel::sql_types::Text, _>(&probe)
        .load(&mut *verify_conn)
        .await
        .expect("verify load");
        assert_eq!(hits[0].n, 0, "rollback did not discard CREATE TABLE");
    }
}
