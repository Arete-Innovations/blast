use std::env;

use diesel::{Connection, PgConnection};
use diesel_migrations::EmbeddedMigrations;

use crate::{
    cata_log,
    database::{auto_migrate, db},
    structs::fuses::registry::FuseRegistry,
    transport::fuses,
};

pub async fn bootstrap(migrations: EmbeddedMigrations) {
    cata_log!(Info, "Starting Catalyst bootstrap...");
    cata_log!(Info, "Running pending migrations");
    let database_url = match env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(e) => {
            cata_log!(Error, format!("DATABASE_URL not set: {}", e));
            panic!("DATABASE_URL not set: {}", e)
        }
    };
    let mut sync_conn = match PgConnection::establish(&database_url) {
        Ok(c) => c,
        Err(e) => {
            cata_log!(Error, format!("Failed to open sync connection for migrations: {}", e));
            panic!("Failed to open sync connection for migrations: {}", e)
        }
    };
    if let Err(e) = auto_migrate::run_pending(&mut sync_conn, migrations) {
        panic!("Migration failed; refusing to start: {}", e);
    }

    cata_log!(Debug, "Initializing database connection pool");
    if let Err(e) = db::init_connection_pool().await {
        cata_log!(Error, format!("Failed to initialize database connection pool: {}", e));
        panic!("Database initialization failed");
    }

    cata_log!(Debug, "Launching fuses scheduler");
    let registry = FuseRegistry::new();
    if let Err(e) = fuses::launch(db::pool().clone(), registry).await {
        cata_log!(Error, format!("Failed to launch fuses scheduler: {}", e));
        panic!("Fuses scheduler launch failed: {}", e);
    }

    cata_log!(Info, "Bootstrap completed successfully");
}
