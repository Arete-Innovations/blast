use std::net::SocketAddr;

use axum::{body::Body, extract::ConnectInfo, http::{Request, StatusCode}, routing::post, Router};
use canonical::transport::http::middleware::rate_limit::{with_auth_rate_limit, AUTH_RATE_LIMIT_MAX};
use tower::ServiceExt;

fn build_app() -> Router {
    let throttled = with_auth_rate_limit(
        Router::<()>::new()
            .route("/path/a", post(|| async { StatusCode::OK }))
            .route("/path/b", post(|| async { StatusCode::OK })),
    );
    throttled
}

fn req(path: &str, ip: &str) -> Request<Body> {
    let addr: SocketAddr = format!("{}:42424", ip).parse().expect("parse addr");
    let mut r = Request::builder().method("POST").uri(path).body(Body::empty()).expect("build req");
    r.extensions_mut().insert(ConnectInfo::<SocketAddr>(addr));
    r
}

#[tokio::test]
async fn exhausts_bucket_then_429() {
    let app = build_app();
    for _ in 0..AUTH_RATE_LIMIT_MAX {
        let resp = app.clone().oneshot(req("/path/a", "10.0.0.1")).await.expect("oneshot ok");
        assert_eq!(resp.status(), StatusCode::OK);
    }
    let resp = app.oneshot(req("/path/a", "10.0.0.1")).await.expect("oneshot ok");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn separate_paths_have_independent_buckets() {
    let app = build_app();
    for _ in 0..AUTH_RATE_LIMIT_MAX {
        let resp = app.clone().oneshot(req("/path/a", "10.0.0.2")).await.expect("oneshot ok");
        assert_eq!(resp.status(), StatusCode::OK);
    }
    let blocked = app.clone().oneshot(req("/path/a", "10.0.0.2")).await.expect("oneshot ok");
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS, "/path/a exhausted");

    let still_open = app.oneshot(req("/path/b", "10.0.0.2")).await.expect("oneshot ok");
    assert_eq!(still_open.status(), StatusCode::OK, "/path/b unaffected by /path/a exhaustion");
}

#[tokio::test]
async fn separate_ips_have_independent_buckets() {
    let app = build_app();
    for _ in 0..AUTH_RATE_LIMIT_MAX {
        let resp = app.clone().oneshot(req("/path/a", "10.0.0.3")).await.expect("oneshot ok");
        assert_eq!(resp.status(), StatusCode::OK);
    }
    let other_ip = app.oneshot(req("/path/a", "10.0.0.4")).await.expect("oneshot ok");
    assert_eq!(other_ip.status(), StatusCode::OK, "different IP gets its own bucket");
}
