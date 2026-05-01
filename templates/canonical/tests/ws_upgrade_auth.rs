use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use canonical::{
    ctx::{Ctx, CtxPool},
    transport::ws::{registry::Registry, router::router},
};
use diesel_async::{
    pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use tower::ServiceExt;

fn build_pool() -> CtxPool {
    let cfg = AsyncDieselConnectionManager::<AsyncPgConnection>::new("postgres://localhost/__catablast_unused__");
    Pool::builder(cfg).max_size(1).build().expect("pool build")
}

fn build_app() -> Router {
    let ctx = Ctx::anonymous(build_pool());
    let registry = Registry::new();
    router(ctx, registry)
}

fn upgrade_request(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method("GET")
        .uri(uri)
        .header("Host", "localhost")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13");
    if let Some(c) = cookie {
        b = b.header("Cookie", c);
    }
    b.body(Body::empty()).expect("build req")
}

#[tokio::test]
async fn ws_upgrade_without_session_returns_401() {
    let app = build_app();
    let resp = app.oneshot(upgrade_request("/ws", None)).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "anonymous /ws upgrade must be rejected");
}

#[tokio::test]
async fn ws_upgrade_with_empty_cookie_returns_401() {
    let app = build_app();
    let resp = app.oneshot(upgrade_request("/ws", Some("session=  "))).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "whitespace-only session cookie counts as anonymous");
}

#[tokio::test]
async fn ws_upgrade_returns_meltdown_envelope_on_anon() {
    let app = build_app();
    let resp = app.oneshot(upgrade_request("/ws", None)).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.expect("read body");
    let text = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(text.contains("\"code\":401"), "envelope must carry 401 code, got: {}", text);
    assert!(text.contains("SessionMissing") || text.contains("session"), "envelope mentions session: {}", text);
}
