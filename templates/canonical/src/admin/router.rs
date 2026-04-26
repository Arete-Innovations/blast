
use std::sync::Arc;

use axum::{
    middleware::from_fn,
    routing::{get, post},
    Extension, Router,
};

use crate::{
    admin::{handlers, AdminConfig},
    middleware::auth_middleware::admin_auth_middleware,
};

pub fn admin_router_with(config: AdminConfig) -> Router {
    let shared = Arc::new(config);
    Router::new()
        .route("/", get(handlers::index))
        .route("/:table/", get(handlers::list).post(handlers::create))
        .route("/:table/new", get(handlers::new_form))
        .route("/:table/:id", get(handlers::detail))
        .route("/:table/:id/edit", post(handlers::update))
        .route("/:table/:id/delete", post(handlers::delete))
        .layer(Extension(shared))
        .layer(from_fn(admin_auth_middleware))
}

pub fn admin_router() -> Router {
    admin_router_with(AdminConfig::default())
}
