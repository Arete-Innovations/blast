use axum::{routing::get, Router, Json};
use serde_json::{json, Value};

async fn home_handler() -> Json<Value> {
    Json(json!({
        "message": "Welcome to Catalyst with Axum!",
        "status": "success",
        "framework": "axum"
    }))
}

async fn health_handler() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "catalyst-api"
    }))
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/welcome", get(home_handler))
}