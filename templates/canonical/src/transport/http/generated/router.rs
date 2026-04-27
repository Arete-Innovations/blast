use axum::Router;

use crate::Ctx;

pub fn router() -> Router<Ctx> {
    Router::new()
}
