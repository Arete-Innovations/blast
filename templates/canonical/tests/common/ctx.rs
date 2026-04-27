use canonical::meltdown::MeltDown;
use diesel_async::{scoped_futures::ScopedBoxFuture, AsyncPgConnection};

use super::harness::{with_test_transaction, TestPool};

pub type UserId = i64;

pub struct TestCtx<'a> {
    pub conn: &'a mut AsyncPgConnection,
    pub session_user_id: Option<UserId>,
}

impl<'a> TestCtx<'a> {
    pub fn new(conn: &'a mut AsyncPgConnection) -> Self {
        Self { conn, session_user_id: None }
    }

    pub fn builder() -> TestCtxBuilder {
        TestCtxBuilder::default()
    }
}

#[derive(Debug, Default, Clone)]
pub struct TestCtxBuilder {
    session_user_id: Option<UserId>,
}

impl TestCtxBuilder {
    pub fn as_user(mut self, user_id: UserId) -> Self {
        self.session_user_id = Some(user_id);
        self
    }

    pub fn anonymous(mut self) -> Self {
        self.session_user_id = None;
        self
    }

    pub fn apply<'a>(self, ctx: &mut TestCtx<'a>) {
        ctx.session_user_id = self.session_user_id;
    }
}

pub async fn run_in_test<'a, T, B, F>(pool: &TestPool, build: B, f: F) -> Result<T, MeltDown>
where
    B: FnOnce(TestCtxBuilder) -> TestCtxBuilder + Send + 'a,
    F: for<'r> FnOnce(&'r mut TestCtx<'r>) -> ScopedBoxFuture<'a, 'r, Result<T, MeltDown>> + Send + 'a,
    T: Send + 'a,
{
    let builder = build(TestCtxBuilder::default());

    with_test_transaction(pool, move |conn| {
        Box::pin(async move {
            let mut ctx = TestCtx::new(conn);
            builder.apply(&mut ctx);
            f(&mut ctx).await
        })
    })
    .await
}
