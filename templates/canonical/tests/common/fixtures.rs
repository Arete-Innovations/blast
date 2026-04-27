
use canonical::meltdown::MeltDown;

use super::ctx::TestCtx;

pub trait Fixture: Sized {
    fn create(
        ctx: &mut TestCtx<'_>,
    ) -> impl std::future::Future<Output = Result<Self, MeltDown>> + Send;
}

pub async fn make_fixture<T: Fixture>(ctx: &mut TestCtx<'_>) -> Result<T, MeltDown> {
    T::create(ctx).await
}

#[macro_export]
macro_rules! fixture {
    (let $name:ident : $ty:ty = $ctx:expr) => {
        let $name: $ty = <$ty as common::fixtures::Fixture>::create($ctx).await?;
    };
}
