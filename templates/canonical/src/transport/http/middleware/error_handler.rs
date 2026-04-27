use std::time::Instant;

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::{cata_log, meltdown::MeltDown};

pub async fn error_handling_middleware(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    match HeaderValue::from_str(&request_id) {
        Ok(val) => {
            headers.insert("X-Request-ID", val);
        }
        Err(e) => {
            cata_log!(Debug, format!("Failed to parse request_id as HeaderValue: {}", e));
        }
    }
    match HeaderValue::from_str(&format!("{}ms", start.elapsed().as_millis())) {
        Ok(val) => {
            headers.insert("X-Response-Time", val);
        }
        Err(e) => {
            cata_log!(Debug, format!("Failed to parse response time as HeaderValue: {}", e));
        }
    }
    
    let status = response.status();
    let duration = start.elapsed();
    
    match status.as_u16() {
        200..=299 => cata_log!(Info, format!("{} {} {} - {}ms", method, uri, status, duration.as_millis())),
        400..=499 => cata_log!(Warning, format!("{} {} {} - {}ms", method, uri, status, duration.as_millis())),
        500..=599 => cata_log!(Error, format!("{} {} {} - {}ms", method, uri, status, duration.as_millis())),
        other => cata_log!(Debug, format!("{} {} {} - {}ms", method, uri, other, duration.as_millis())),
    }
    
    response
}

pub async fn panic_handler(err: Box<dyn std::any::Any + Send + 'static>) -> Response {
    let error_id = Uuid::new_v4().to_string();

    let message = extract_panic_message(&err);

    cata_log!(Error, format!("PANIC [{}]: {}", error_id, message));

    let error_response = json!({
        "error": {
            "code": 500,
            "type": "InternalServerError",
            "message": "An internal server error occurred",
            "error_id": error_id
        }
    });

    (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
}

pub fn create_error_response(
    status: StatusCode,
    error_type: &str,
    message: &str,
    details: Option<serde_json::Value>,
) -> Response {
    let mut error_json = json!({
        "error": {
            "code": status.as_u16(),
            "type": error_type,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    });

    details.map(|det| {
        error_json["error"]["details"] = det;
    });

    (status, Json(error_json)).into_response()
}

fn extract_panic_message(err: &Box<dyn std::any::Any + Send + 'static>) -> String {
    let Some(s) = err.downcast_ref::<String>() else {
        let Some(s) = err.downcast_ref::<&str>() else {
            return "Unknown panic occurred".to_string();
        };
        return s.to_string();
    };
    s.clone()
}

pub fn validation_error(field: &str, message: &str) -> MeltDown {
    MeltDown::validation_failed(message)
        .with_context("field", field)
        .with_context("error_type", "validation")
}

pub fn auth_error(reason: &str) -> MeltDown {
    MeltDown::new(crate::meltdown::MeltType::Unauthorized, "Authentication required")
        .with_user_message("Please log in to access this resource")
        .with_context("reason", reason)
}

pub fn authz_error(reason: &str) -> MeltDown {
    MeltDown::new(crate::meltdown::MeltType::Forbidden, "Access denied")
        .with_user_message("You don't have permission to access this resource")
        .with_context("reason", reason)
}

pub fn not_found_error(resource: &str) -> MeltDown {
    MeltDown::record_not_found(resource)
        .with_user_message(&format!("{} not found", resource))
}

pub fn rate_limit_error() -> MeltDown {
    MeltDown::new(crate::meltdown::MeltType::BadRequest, "Rate limit exceeded")
        .with_user_message("Too many requests. Please try again later.")
        .with_context("error_type", "rate_limit")
}