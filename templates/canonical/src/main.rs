use axum::{extract::DefaultBodyLimit, middleware::from_fn, Router};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::{ServeDir, ServeFile}, trace::TraceLayer};

mod bootstrap;
mod database;
mod meltdown;
mod middleware;
mod models;
mod routes;
mod services;
mod sessions;
mod structs;

mod logger;

use bootstrap::bootstrap;
use middleware::*;
use routes::*;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("src/database/migrations");

#[tokio::main]
async fn main() {
    logger::init_tracing();

    bootstrap(MIGRATIONS).await;
    cata_log!(Info, "Starting Axum server...");

    let app = create_app().await;

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    cata_log!(Info, format!("Server listening on {}", addr));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_app() -> Router {
    let api_routes = Router::new()
        .merge(auth::routes())         
        .merge(home::routes())         
        .merge(user_api::routes())     
        .merge(protected::routes())    
        .merge(error_demo::routes());  

    // SPA fallback: serve frontend/dist/ as static assets; any unmatched path
    // (vue-router history-mode deep links) falls through to index.html.
    // API and WS routes registered above take precedence — they never reach here.
    let spa_service = ServeDir::new("frontend/dist")
        .fallback(ServeFile::new("frontend/dist/index.html"));

    let mut app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(spa_service);

    app = app.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .layer(DefaultBodyLimit::max(1024 * 1024))
            .layer(from_fn(error_handling_middleware))
            .layer(from_fn(api_logger_middleware)),
    );

    cata_log!(Info, "Development mode: cache control disabled");

    app
}
