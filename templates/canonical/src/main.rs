use std::net::SocketAddr;

use axum::{extract::DefaultBodyLimit, middleware::from_fn, Router};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

mod bootstrap;
mod crank;
mod ctx;
mod database;
mod flows;
mod meltdown;
mod models;
mod routines;
mod seeds;
mod services;
mod structs;
mod time;
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

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            cata_log!(Error, format!("failed to bind {}: {}", addr, e));
            std::process::exit(1);
        }
    };
    match axum::serve(listener, app).await {
        Ok(()) => {}
        Err(e) => {
            cata_log!(Error, format!("axum serve exited: {}", e));
            std::process::exit(1);
        }
    }
}

async fn create_app() -> Router {
    let ctx = Ctx::anonymous(database::db::pool().clone());
    let api_routes = transport::http::router(ctx);

    let static_files = ServeDir::new("frontend/dist").not_found_service(ServeFile::new("frontend/dist/index.html"));

    let app = Router::new().nest("/api", api_routes).fallback_service(static_files).layer(
        ServiceBuilder::new()
            .layer(transport::http::middleware::trace::make_trace_layer())
            .layer(CorsLayer::permissive())
            .layer(DefaultBodyLimit::max(1024 * 1024))
            .layer(from_fn(error_handling_middleware))
            .layer(from_fn(api_logger_middleware)),
    );

    app
}
