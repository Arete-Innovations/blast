use axum::{middleware::from_fn_with_state, Router};

use crate::transport::http::middleware::auth::request_ctx_middleware;
use crate::Ctx;

pub fn router(ctx: Ctx) -> Router {
    crate::transport::http::auth::router()
        .merge(crate::transport::http::healthz::router())
        .merge(crate::transport::http::generated::router())
        .layer(from_fn_with_state(ctx.clone(), request_ctx_middleware))
        .with_state(ctx)
}
