use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

use crate::{cata_log, flows::sessions::resolve, meltdown::*, Ctx};

fn extract_token(request: &Request) -> Option<String> {
    let value = request.headers().get(AUTHORIZATION)?;
    let header = match value.to_str() {
        Ok(h) => h,
        Err(e) => {
            cata_log!(Debug, format!("non-utf8 authorization header: {}", e));
            return None;
        }
    };
    let token = header.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

pub async fn request_ctx_middleware(State(ctx): State<Ctx>, mut request: Request, next: Next) -> Result<Response, MeltDown> {
    let request_ctx = match extract_token(&request) {
        None => ctx.clone(),
        Some(raw_token) => {
            let session_ctx = resolve::run(&ctx, &raw_token).await?;
            request.extensions_mut().insert(session_ctx.clone());
            Ctx::with_session(ctx.pool().clone(), session_ctx)
        }
    };
    request.extensions_mut().insert(request_ctx);
    Ok(next.run(request).await)
}
