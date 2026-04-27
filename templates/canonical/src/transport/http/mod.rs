use axum::Router;

use crate::Ctx;

pub mod custom;
pub mod generated;
pub mod list_query;
pub mod middleware;

pub fn router(ctx: Ctx) -> Router {
    custom::router()
        .merge(generated::router())
        .with_state(ctx)
}
