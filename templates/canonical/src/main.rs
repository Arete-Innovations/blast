use std::net::SocketAddr;

use axum::{extract::DefaultBodyLimit, middleware::from_fn, Router};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use leptos::prelude::*;
use leptos_axum::{file_and_error_handler, generate_route_list, LeptosRoutes};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

mod bootstrap;
mod crank;
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
use transport::leptos::app::{shell, App};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("src/database/migrations");

#[tokio::main]
async fn main() {
    if let Err(e) = dotenv::dotenv() {
        eprintln!("Could not load .env file: {}", e);
    }
    cata_log::init_tracing();

    bootstrap(MIGRATIONS).await;
    cata_log!(Info, "Starting Axum server...");

    let leptos_conf = match leptos::prelude::get_configuration(None) {
        Ok(c) => c,
        Err(e) => {
            cata_log!(Error, format!("leptos config failed: {}", e));
            std::process::exit(1);
        }
    };
    let leptos_options = leptos_conf.leptos_options;
    let addr = leptos_options.site_addr;

    let app = create_app(leptos_options).await;

    cata_log!(Info, format!("Server listening on {}", addr));

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            cata_log!(Error, format!("failed to bind {}: {}", addr, e));
            std::process::exit(1);
        }
    };
    match axum::serve(listener, app.into_make_service()).await {
        Ok(()) => {}
        Err(e) => {
            cata_log!(Error, format!("axum serve exited: {}", e));
            std::process::exit(1);
        }
    }
}

async fn create_app(leptos_options: LeptosOptions) -> Router {
    let ctx = Ctx::anonymous(database::db::pool().clone());
    let api_routes = transport::http::router(ctx);

    let routes = generate_route_list(App);
    let opts_for_leptos = leptos_options.clone();

    let leptos_router: Router<LeptosOptions> = Router::new()
        .leptos_routes(&leptos_options, routes, move || shell(opts_for_leptos.clone()))
        .fallback(file_and_error_handler::<LeptosOptions, _>(shell));

    let leptos_router_stateless: Router = leptos_router.with_state(leptos_options);

    let app = Router::new()
        .nest("/api", api_routes)
        .merge(leptos_router_stateless)
        .layer(
            ServiceBuilder::new()
                .layer(transport::http::middleware::trace::make_trace_layer())
                .layer(CorsLayer::permissive())
                .layer(DefaultBodyLimit::max(1024 * 1024))
                .layer(from_fn(error_handling_middleware)),
        );

    app
}
