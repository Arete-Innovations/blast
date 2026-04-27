use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

use crate::{
    cata_log,
    flows::sessions::resolve,
    meltdown::*,
    structs::auth::Role,
    Ctx,
};

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

pub async fn session_auth_middleware(
    State(ctx): State<Ctx>,
    mut request: Request,
    next: Next,
) -> Result<Response, MeltDown> {
    let raw_token = extract_token(&request).ok_or_else(|| {
        cata_log!(Debug, "Missing bearer token (no Authorization header)");
        MeltDown::session_missing()
    })?;

    let session_ctx = resolve::run(&ctx, &raw_token).await?;
    request.extensions_mut().insert(session_ctx);

    Ok(next.run(request).await)
}

pub async fn admin_auth_middleware(
    State(ctx): State<Ctx>,
    mut request: Request,
    next: Next,
) -> Result<Response, MeltDown> {
    let raw_token = extract_token(&request).ok_or_else(|| {
        cata_log!(Debug, "Missing bearer token for admin route");
        MeltDown::session_missing()
    })?;

    let session_ctx = resolve::run(&ctx, &raw_token).await?;

    if session_ctx.role != Role::Admin {
        cata_log!(Warning, format!("Non-admin user attempted admin access (user_id: {})", session_ctx.user_id));
        return Err(MeltDown::new(MeltType::Forbidden, "Admin access required"));
    }

    cata_log!(Debug, format!("Authenticated admin user (user_id: {})", session_ctx.user_id));
    request.extensions_mut().insert(session_ctx);

    Ok(next.run(request).await)
}
