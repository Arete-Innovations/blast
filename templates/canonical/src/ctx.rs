use diesel_async::{
    pooled_connection::deadpool::{Object, Pool},
    scoped_futures::ScopedBoxFuture,
    AsyncConnection, AsyncPgConnection,
};

use crate::{
    meltdown::*,
    structs::auth::{Role, SessionContext},
};

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

    pub fn system(pool: CtxPool) -> Self {
        let session = SessionContext::new(0, 0, Role::Admin, "");
        Self {
            pool,
            session: Some(session),
            request_id: None,
        }
    }

    pub fn with_session(pool: CtxPool, session: SessionContext) -> Self {
        Self {
            pool,
            session: Some(session),
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    pub async fn conn(&self) -> Result<Object<AsyncPgConnection>, MeltDown> {
        self.pool.get().await.map_err(|e| MeltDown::db_connection(format!("ctx: failed to get pool conn: {}", e)))
    }

    pub async fn transaction<'a, T, F>(&self, f: F) -> Result<T, MeltDown>
    where
        F: for<'r> FnOnce(&'r mut AsyncPgConnection) -> ScopedBoxFuture<'a, 'r, Result<T, MeltDown>> + Send + 'a,
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
        self.session.as_ref().ok_or_else(MeltDown::session_missing)
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
