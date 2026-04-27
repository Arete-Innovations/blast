use std::{
    env,
    sync::OnceLock,
};

use diesel_async::{
    pooled_connection::{
        deadpool::Pool,
        AsyncDieselConnectionManager,
    },
    AsyncConnection, AsyncPgConnection,
};
use dotenv::dotenv;

use crate::{cata_log, meltdown::*};

const MAX_POOL_SIZE: usize = 20;

pub type DbPool = Pool<AsyncPgConnection>;

static DB_POOL: OnceLock<DbPool> = OnceLock::new();

pub async fn init_connection_pool() -> Result<(), MeltDown> {
    match dotenv() {
        Ok(path) => cata_log!(Debug, format!("Loaded .env from {}", path.display())),
        Err(e) => cata_log!(Debug, format!("Failed to load .env: {}", e)),
    }

    if DB_POOL.get().is_some() {
        cata_log!(Debug, "Connection pool already initialized");
        return Ok(());
    }

    let database_url = env::var("DATABASE_URL").map_err(|e| MeltDown::new(MeltType::EnvironmentError, format!("DATABASE_URL not set: {}", e)))?;

    cata_log!(Info, "Initializing database connection pool");

    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);

    let pool = Pool::builder(config)
        .max_size(MAX_POOL_SIZE)
        .build()
        .map_err(|e| MeltDown::db_connection(format!("Failed to create connection pool: {}", e)))?;

    let probe = pool.get().await
        .map_err(|e| MeltDown::db_connection(format!("Failed to verify connection pool: {}", e)))?;
    drop(probe);

    match DB_POOL.set(pool) {
        Ok(()) => {
            cata_log!(Info, "Database connection pool initialized successfully");
            Ok(())
        }
        Err(returned) => {
            drop(returned);
            Err(MeltDown::new(MeltType::ConfigurationError, "Failed to set connection pool: already initialized"))
        }
    }
}

async fn get_conn_from_pool() -> Result<diesel_async::pooled_connection::deadpool::Object<AsyncPgConnection>, MeltDown> {
    let pool = DB_POOL.get().ok_or_else(|| MeltDown::new(MeltType::DatabaseConnection, "Database pool not initialized"))?;

    pool.get().await.map_err(|e| MeltDown::db_connection(format!("Failed to get connection from pool: {}", e)))
}

pub fn pool() -> &'static DbPool {
    let Some(p) = DB_POOL.get() else {
        cata_log!(Error, "pool not initialized — call init_connection_pool() first");
        panic!("pool not initialized — call init_connection_pool() first");
    };
    p
}

pub async fn acquire_conn() -> Result<diesel_async::pooled_connection::deadpool::Object<AsyncPgConnection>, MeltDown> {
    get_conn_from_pool().await
}

pub async fn establish_connection() -> Result<AsyncPgConnection, MeltDown> {
    let database_url = env::var("DATABASE_URL").map_err(|e| MeltDown::new(MeltType::EnvironmentError, format!("DATABASE_URL not set: {}", e)))?;

    AsyncPgConnection::establish(&database_url)
        .await
        .map_err(|e| MeltDown::db_connection(format!("Error connecting to database: {}", e)))
}
