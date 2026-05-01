use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::{ConnectInfo, Request},
    middleware::{from_fn, Next},
    response::{IntoResponse, Response},
    Router,
};

use crate::{meltdown::MeltDown, structs::services::rate_limit::RateLimit};

pub const AUTH_RATE_LIMIT_MAX: u32 = 5;
pub const AUTH_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

pub fn with_auth_rate_limit<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    let limiter: Arc<RateLimit> = Arc::new(RateLimit::new());
    router.layer(from_fn(move |ConnectInfo(addr): ConnectInfo<SocketAddr>, req: Request, next: Next| {
        let limiter = Arc::clone(&limiter);
        async move {
            let key = format!("{}:{}", req.uri().path(), addr.ip());
            if !limiter.check_and_consume(&key, AUTH_RATE_LIMIT_MAX, AUTH_RATE_LIMIT_WINDOW) {
                return MeltDown::too_many_requests(AUTH_RATE_LIMIT_WINDOW).into_response();
            }
            next.run(req).await
        }
    }))
}
