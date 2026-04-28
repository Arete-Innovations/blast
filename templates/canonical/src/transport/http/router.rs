use axum::Router;

use crate::Ctx;

pub fn router(ctx: Ctx) -> Router {
    crate::transport::http::auth::router(ctx.clone())
        .merge(crate::transport::http::healthz::router())
        .merge(crate::transport::http::generated::router())
        .with_state(ctx)
}
