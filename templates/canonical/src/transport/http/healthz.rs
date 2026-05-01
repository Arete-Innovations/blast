use std::time::Duration;

use axum::{
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::time::timeout;

use crate::{cata_log, meltdown::MeltDown, Ctx};

const POOL_TIMEOUT: Duration = Duration::from_millis(500);

pub fn router() -> Router<Ctx> {
    Router::new().route("/healthz", get(handler))
}

async fn handler(Extension(ctx): Extension<Ctx>) -> Response {
    match timeout(POOL_TIMEOUT, ctx.conn()).await {
        Ok(Ok(conn)) => {
            drop(conn);
            (StatusCode::OK, "ok").into_response()
        }
        Ok(Err(e)) => {
            cata_log!(Warning, format!("healthz pool acquire failed: {}", e));
            e.into_response()
        }
        Err(elapsed) => {
            cata_log!(Warning, format!("healthz pool acquire timed out: {}", elapsed));
            MeltDown::db_connection("pool acquire timed out after 500ms").into_response()
        }
    }
}
