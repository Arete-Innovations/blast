use axum::{extract::Request, middleware::Next, response::Response};

use crate::cata_log;

pub async fn api_logger_middleware(request: Request, next: Next) -> Response {
    let uri = request.uri().clone();
    let method = request.method().clone();
    
    cata_log!(Debug, format!("API Request: {} {}", method, uri));
    
    let response = next.run(request).await;
    let status = response.status();
    
    cata_log!(Debug, format!("API Response: {} {} -> {}", method, uri, status));
    
    response
}
