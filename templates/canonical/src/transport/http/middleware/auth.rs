use axum::{
    extract::{Request, State},
    http::header::{AUTHORIZATION, COOKIE},
    middleware::Next,
    response::Response,
};

use crate::{cata_log, flows::sessions::resolve, meltdown::*, transport::http::auth::SESSION_COOKIE, Ctx};

fn extract_bearer(request: &Request) -> Option<String> {
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

fn extract_cookie(request: &Request) -> Option<String> {
    let value = request.headers().get(COOKIE)?;
    let header = match value.to_str() {
        Ok(h) => h,
        Err(e) => {
            cata_log!(Debug, format!("non-utf8 cookie header: {}", e));
            return None;
        }
    };
    for entry in header.split(';') {
        let trimmed = entry.trim();
        let Some((name, val)) = trimmed.split_once('=') else {
            continue;
        };
        if name == SESSION_COOKIE {
            let v = val.trim();
            if v.is_empty() {
                return None;
            }
            return Some(v.to_string());
        }
    }
    None
}

fn extract_token(request: &Request) -> Option<String> {
    match extract_cookie(request) {
        Some(t) => Some(t),
        None => extract_bearer(request),
    }
}

pub async fn request_ctx_middleware(State(ctx): State<Ctx>, mut request: Request, next: Next) -> Result<Response, MeltDown> {
    let request_ctx = match extract_token(&request) {
        None => ctx.clone(),
        Some(raw_token) => match resolve::run(&ctx, &raw_token).await {
            Ok(session_ctx) => {
                tracing::Span::current().record("user_id", session_ctx.user_id);
                request.extensions_mut().insert(session_ctx.clone());
                Ctx::with_session(ctx.pool().clone(), session_ctx)
            }
            Err(err) => {
                if err.is_permanent() {
                    cata_log!(Debug, format!("stale/invalid session cookie ignored: {}", err));
                    ctx.clone()
                } else {
                    cata_log!(Warning, format!("transient error resolving session: {}", err));
                    return Err(err);
                }
            }
        },
    };
    request.extensions_mut().insert(request_ctx);
    Ok(next.run(request).await)
}
