use std::env;

use diesel::{Connection, PgConnection};
use diesel_migrations::EmbeddedMigrations;

use crate::{
    cata_log,
    database::{auto_migrate, db},
    seeds,
};

pub async fn bootstrap(migrations: EmbeddedMigrations) {
    cata_log!(Info, "Starting Catalyst bootstrap...");

    if let Err(e) = dotenv::dotenv() {
        cata_log!(Warning, format!("Could not load .env file: {}", e));
    }

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

    match db::acquire_conn().await {
        Ok(mut conn) => {
            if let Err(e) = seeds::ensure_admin(&mut conn).await {
                cata_log!(Error, format!("Admin seed failed: {}", e));
            }
        }
        Err(e) => cata_log!(Error, format!("Could not acquire conn for admin seed: {}", e)),
    }

    cata_log!(Info, "Bootstrap completed successfully");
}
