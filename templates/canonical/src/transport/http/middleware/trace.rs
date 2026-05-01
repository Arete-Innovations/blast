use std::time::Duration;

use axum::http::{HeaderName, HeaderValue, Request};
use tower_http::trace::{MakeSpan, OnResponse, TraceLayer};
use tracing::Span;
use uuid::Uuid;

use crate::{
    cata_log,
    structs::middleware::trace::{CatalystMakeSpan, CatalystOnResponse},
};

pub fn make_trace_layer() -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>, CatalystMakeSpan, (), CatalystOnResponse> {
    TraceLayer::new_for_http().make_span_with(CatalystMakeSpan).on_response(CatalystOnResponse).on_request(())
}

impl<B> MakeSpan<B> for CatalystMakeSpan {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        let method = request.method().as_str();
        let uri = request.uri().to_string();

        if cfg!(feature = "prod") {
            let request_id = Uuid::new_v4().to_string();
            tracing::info_span!(
                "request",
                request_id = %request_id,
                method     = %method,
                uri        = %uri,
                status     = tracing::field::Empty,
                latency_ms = tracing::field::Empty,
            )
        } else {
            tracing::info_span!(
                "request",
                method     = %method,
                uri        = %uri,
                status     = tracing::field::Empty,
                latency_ms = tracing::field::Empty,
            )
        }
    }
}

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
    match HeaderValue::from_str(id) {
        Ok(v) => Some(v),
        Err(e) => {
            cata_log!(Warning, format!("invalid request id header value: {}", e));
            None
        }
    }
}
