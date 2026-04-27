use axum::Router;

use crate::Ctx;

pub fn router(ctx: Ctx) -> Router {
    super::custom::router(ctx.clone())
        .merge(super::generated::router())
        .with_state(ctx)
}
