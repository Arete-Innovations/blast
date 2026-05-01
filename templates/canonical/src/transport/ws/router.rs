use std::sync::Arc;

use axum::{middleware::from_fn_with_state, routing::get, Router};

use crate::{
    transport::{
        http::middleware::auth::request_ctx_middleware,
        ws::{registry::Registry, route::ws_upgrade},
    },
    Ctx,
};

pub fn router(ctx: Ctx, registry: Arc<Registry>) -> Router {
    Router::new()
        .route("/ws", get(ws_upgrade))
        .layer(from_fn_with_state(ctx, request_ctx_middleware))
        .with_state(registry)
}
