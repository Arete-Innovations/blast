use axum::{extract::Path, routing::get, Router, Json};
use serde_json::{json, Value};

use crate::{
    middleware::{validation_error, auth_error, not_found_error},
    meltdown::{MeltDown, MeltType},
};

async fn demo_validation_error() -> Result<Json<Value>, MeltDown> {
    Err(validation_error("email", "Invalid email format"))
}

async fn demo_auth_error() -> Result<Json<Value>, MeltDown> {
    Err(auth_error("missing_token"))
}

async fn demo_not_found_error() -> Result<Json<Value>, MeltDown> {
    Err(not_found_error("User"))
}

async fn demo_database_error() -> Result<Json<Value>, MeltDown> {
    Err(MeltDown::new(MeltType::DatabaseConnection, "Failed to connect to database")
        .with_user_message("Service temporarily unavailable")
        .with_context("database", "postgres")
        .with_context("retry_after", "30s"))
}

async fn demo_success() -> Json<Value> {
    Json(json!({
        "message": "Success! This demonstrates a working endpoint.",
        "status": "ok"
    }))
}

async fn demo_custom_error(Path(error_type): Path<String>) -> Result<Json<Value>, MeltDown> {
    match error_type.as_str() {
        "validation" => Err(validation_error("username", "Username must be at least 3 characters")),
        "auth" => Err(auth_error("invalid_credentials")),
        "not_found" => Err(not_found_error("Post")),
        "database" => Err(MeltDown::db_connection("Connection timeout")),
        "server" => Err(MeltDown::new(MeltType::Unexpected("demo_server_error".to_string()), "Something went wrong")
            .with_user_message("An unexpected error occurred")),
        _ => Ok(Json(json!({
            "message": "Valid error types: validation, auth, not_found, database, server",
            "requested": error_type
        })))
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/demo/errors/validation", get(demo_validation_error))
        .route("/demo/errors/auth", get(demo_auth_error))
        .route("/demo/errors/not_found", get(demo_not_found_error))
        .route("/demo/errors/database", get(demo_database_error))
        .route("/demo/errors/success", get(demo_success))
        .route("/demo/errors/:type", get(demo_custom_error))
}