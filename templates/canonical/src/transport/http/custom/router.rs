use axum::Router;

use crate::Ctx;

pub fn router(ctx: Ctx) -> Router<Ctx> {
    super::auth::router(ctx).merge(super::healthz::router())
}
