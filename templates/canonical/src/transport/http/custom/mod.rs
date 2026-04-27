use axum::Router;

use crate::Ctx;

pub mod auth;
pub mod healthz;

pub fn router() -> Router<Ctx> {
    auth::router().merge(healthz::router())
}
