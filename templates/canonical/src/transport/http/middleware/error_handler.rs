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

fn extract_panic_message(err: &Box<dyn std::any::Any + Send + 'static>) -> String {
    let Some(s) = err.downcast_ref::<String>() else {
        let Some(s) = err.downcast_ref::<&str>() else {
            return "Unknown panic occurred".to_string();
        };
        return s.to_string();
    };
    s.clone()
}