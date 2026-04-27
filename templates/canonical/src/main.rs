use axum::{extract::DefaultBodyLimit, middleware::from_fn, Router};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

mod bootstrap;
mod ctx;
mod database;
mod flows;
mod meltdown;
mod models;
mod routines;
mod services;
mod structs;
mod transport;

mod cata_log;

use bootstrap::bootstrap;
use ctx::Ctx;
use transport::http::middleware::*;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("src/database/migrations");

#[tokio::main]
async fn main() {
    cata_log::init_tracing();

    bootstrap(MIGRATIONS).await;
    cata_log!(Info, "Starting Axum server...");

    let app = create_app().await;

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    cata_log!(Info, format!("Server listening on {}", addr));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_app() -> Router {
    let ctx = Ctx::anonymous(database::db::pool().clone());
    let api_routes = transport::http::router(ctx);

    let app = Router::new().nest("/api", api_routes).layer(
        ServiceBuilder::new()
            .layer(transport::http::middleware::trace::make_trace_layer())
            .layer(CorsLayer::permissive())
            .layer(DefaultBodyLimit::max(1024 * 1024))
            .layer(from_fn(error_handling_middleware))
            .layer(from_fn(api_logger_middleware)),
    );

    app
}
