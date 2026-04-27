use axum::Router;

use crate::Ctx;

pub fn router(ctx: Ctx) -> Router {
    super::auth::router(ctx.clone()).merge(super::healthz::router()).merge(super::generated::router()).with_state(ctx)
}
