
use std::time::Duration;

use axum::{
    http::{HeaderName, HeaderValue, Request},
    routing::get,
    Router,
};
use tower_http::trace::{MakeSpan, OnResponse, TraceLayer};
use tracing::Span;
use uuid::Uuid;

pub fn make_trace_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    CatalystMakeSpan,
    (),
    CatalystOnResponse,
> {
    TraceLayer::new_for_http()
        .make_span_with(CatalystMakeSpan)
        .on_response(CatalystOnResponse)
        .on_request(())
}

#[derive(Clone)]
pub struct CatalystMakeSpan;

impl<B> MakeSpan<B> for CatalystMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let request_id = Uuid::new_v4().to_string();
        let method = request.method().as_str();
        let uri = request.uri().to_string();

        tracing::info_span!(
            "request",
            request_id = %request_id,
            method     = %method,
            uri        = %uri,
            status     = tracing::field::Empty,
            latency_ms = tracing::field::Empty,
        )
    }
}

#[derive(Clone)]
pub struct CatalystOnResponse;

impl<B> OnResponse<B> for CatalystOnResponse {
    fn on_response(self, response: &axum::http::Response<B>, latency: Duration, span: &Span) {
        span.record("status", response.status().as_u16());
        span.record("latency_ms", latency.as_millis() as u64);
    }
}

pub fn request_id_header() -> HeaderName {
    HeaderName::from_static("x-request-id")
}

pub fn request_id_header_value(id: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(id).ok()
}


pub fn healthz_route() -> Router {
    Router::new().route("/healthz", get(healthz_handler))
}

async fn healthz_handler() -> axum::response::Response {
    use axum::{
        http::StatusCode,
        response::IntoResponse,
    };
    use tokio::time::timeout;

    const POOL_TIMEOUT: Duration = Duration::from_millis(500);

    let result = timeout(POOL_TIMEOUT, crate::database::db::acquire_conn()).await;

    match result {
        Ok(Ok(_conn)) => (StatusCode::OK, "ok").into_response(),
        Ok(Err(meltdown)) => meltdown.into_response(),
        Err(_elapsed) => {
            crate::meltdown::MeltDown::db_connection("pool acquire timed out after 500ms")
                .into_response()
        }
    }
}
