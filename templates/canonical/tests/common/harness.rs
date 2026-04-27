
use diesel::sql_query;
use diesel_async::{
    pooled_connection::deadpool::Pool,
    scoped_futures::ScopedBoxFuture,
    AsyncPgConnection, RunQueryDsl,
};

use canonical::meltdown::{MeltDown, MeltType};

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
