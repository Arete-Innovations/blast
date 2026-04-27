
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::scoped_futures::ScopedBoxFuture;
use diesel_async::{AsyncConnection, AsyncPgConnection};

use crate::meltdown::*;
use crate::structs::auth::{Role, SessionContext};

pub type CtxPool = Pool<AsyncPgConnection>;

#[derive(Clone)]
pub struct Ctx {
    pool: CtxPool,
    session: Option<SessionContext>,
    request_id: Option<String>,
}

impl Ctx {
    pub fn anonymous(pool: CtxPool) -> Self {
        Self { pool, session: None, request_id: None }
    }

    pub fn with_session(pool: CtxPool, session: SessionContext) -> Self {
        Self { pool, session: Some(session), request_id: None }
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    pub async fn conn(&self) -> Result<Object<AsyncPgConnection>, MeltDown> {
        self.pool
            .get()
            .await
            .map_err(|e| MeltDown::db_connection(format!("ctx: failed to get pool conn: {}", e)))
    }

    pub async fn transaction<'a, T, F>(&self, f: F) -> Result<T, MeltDown>
    where
        F: for<'r> FnOnce(&'r mut AsyncPgConnection) -> ScopedBoxFuture<'a, 'r, Result<T, MeltDown>>
            + Send
            + 'a,
        T: Send + 'a,
    {
        let mut conn = self.conn().await?;
        AsyncConnection::transaction::<T, MeltDown, _>(&mut *conn, f).await
    }

    pub fn pool(&self) -> &CtxPool {
        &self.pool
    }

    pub fn session(&self) -> Option<&SessionContext> {
        self.session.as_ref()
    }

    pub fn require_session(&self) -> Result<&SessionContext, MeltDown> {
        self.session
            .as_ref()
            .ok_or_else(MeltDown::session_missing)
    }

    pub fn role(&self) -> Option<Role> {
        self.session.as_ref().map(|s| s.role)
    }

    pub fn is_admin(&self) -> bool {
        self.role() == Some(Role::Admin)
    }

    pub fn require_role(&self, required: Role) -> Result<(), MeltDown> {
        if self.role() == Some(required) {
            Ok(())
        } else {
            Err(MeltDown::insufficient_permissions())
        }
    }

    pub fn require_admin(&self) -> Result<(), MeltDown> {
        self.require_role(Role::Admin)
    }

    pub fn require_any(&self, allowed: &[Role]) -> Result<(), MeltDown> {
        let role = self.role().ok_or_else(MeltDown::session_missing)?;
        if allowed.contains(&role) {
            Ok(())
        } else {
            Err(MeltDown::insufficient_permissions())
        }
    }

    pub fn session_user_id(&self) -> Option<i64> {
        self.session.as_ref().map(|s| s.user_id)
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sql_query;
    use diesel_async::pooled_connection::AsyncDieselConnectionManager;
    use diesel_async::scoped_futures::ScopedFutureExt;
    use diesel_async::RunQueryDsl;

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
}
