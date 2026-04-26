use axum::{extract::Request, middleware::Next, response::Response};

use crate::cata_log;

pub async fn cache_control_middleware(request: Request, next: Next) -> Response {
    cata_log!(Debug, "Cache control middleware activated");
    
    let mut response = next.run(request).await;
    
    response.headers_mut().insert(
        "Cache-Control", 
        "public, max-age=3600".parse().unwrap()
    );
    
    response
}