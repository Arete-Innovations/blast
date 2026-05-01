use std::sync::Arc;

use axum::{
    extract::Request,
    middleware::{from_fn, from_fn_with_state, Next},
    response::Response,
    routing::get,
    Router,
};

use crate::{
    meltdown::MeltDown,
    transport::{
        http::middleware::auth::request_ctx_middleware,
        ws::{registry::Registry, route::ws_upgrade},
    },
    Ctx,
};

pub fn router(ctx: Ctx, registry: Arc<Registry>) -> Router {
    Router::new()
        .route("/ws", get(ws_upgrade))
        .layer(from_fn(require_authenticated_ctx))
        .layer(from_fn_with_state(ctx, request_ctx_middleware))
        .with_state(registry)
}

async fn require_authenticated_ctx(request: Request, next: Next) -> Result<Response, MeltDown> {
    let authed = request.extensions().get::<Ctx>().is_some_and(|c| c.session().is_some());
    if authed {
        Ok(next.run(request).await)
    } else {
        Err(MeltDown::session_missing())
    }
}
