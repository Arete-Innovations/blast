use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{
    cata_log,
    flows::custom::sessions::resolve,
    meltdown::*,
    structs::auth::{Role, SessionContext},
    Ctx,
};

pub struct AdminGuard(pub SessionContext);

#[async_trait]
impl FromRequestParts<Ctx> for AdminGuard {
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, ctx: &Ctx) -> Result<Self, Self::Rejection> {
        let session_ctx = extract_session(parts, ctx).await?;
        if session_ctx.role == Role::Admin {
            Ok(AdminGuard(session_ctx))
        } else {
            Err(MeltDown::new(MeltType::Forbidden, "Insufficient permissions to access admin area"))
        }
    }
}

pub struct UserGuard(pub SessionContext);

#[async_trait]
impl FromRequestParts<Ctx> for UserGuard {
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, ctx: &Ctx) -> Result<Self, Self::Rejection> {
        let session_ctx = extract_session(parts, ctx).await?;
        Ok(UserGuard(session_ctx))
    }
}

async fn extract_session(parts: &Parts, ctx: &Ctx) -> Result<SessionContext, MeltDown> {
    let Some(value) = parts.headers.get(AUTHORIZATION) else {
        return Err(MeltDown::session_missing());
    };
    let header_str = match value.to_str() {
        Ok(s) => s,
        Err(e) => {
            cata_log!(Debug, format!("non-utf8 authorization header: {}", e));
            return Err(MeltDown::session_missing());
        }
    };
    let raw_token = header_str
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(MeltDown::session_missing)?;

    resolve::run(ctx, &raw_token).await
}

pub struct Referer(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for Referer
where
    S: Send + Sync,
{
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(h) = parts.headers.get("Referer") else {
            return Err(MeltDown::new(MeltType::BadRequest, "Missing Referer header"));
        };
        let referer = h.to_str().map_err(|e| {
            MeltDown::new(MeltType::BadRequest, format!("Referer header parse: {}", e))
        })?;
        Ok(Referer(referer.to_string()))
    }
}
